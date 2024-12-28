use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{
    docker::{CROSS_IMAGE, DEFAULT_IMAGE_VERSION},
    errors::*,
    shell::MessageInfo,
    TargetTriple,
};

use super::Engine;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Image {
    pub name: String,
    // The toolchain triple the image is built for
    pub platform: ImagePlatform,
}

impl std::fmt::Display for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PossibleImage {
    #[serde(rename = "name")]
    pub reference: ImageReference,
    // The toolchain triple the image is built for
    pub toolchain: Vec<ImagePlatform>,
}

impl PossibleImage {
    pub fn to_definite_with(&self, engine: &Engine, msg_info: &mut MessageInfo) -> Result<Image> {
        let ImageReference::Name(name) = self.reference.clone() else {
            eyre::bail!("cannot make definite Image from unqualified PossibleImage");
        };

        if self.toolchain.is_empty() {
            Ok(Image {
                name,
                platform: ImagePlatform::DEFAULT,
            })
        } else {
            let platform = if self.toolchain.len() == 1 {
                self.toolchain.first().expect("should contain at least one")
            } else {
                let same_arch = self
                    .toolchain
                    .iter()
                    .filter(|platform| {
                        &platform.architecture
                            == engine.arch.as_ref().unwrap_or(&Architecture::Amd64)
                    })
                    .collect::<Vec<_>>();

                if same_arch.len() == 1 {
                    // pick the platform with the same architecture
                    same_arch.first().expect("should contain one element")
                } else if let Some(platform) = same_arch
                    .iter()
                    .find(|platform| &platform.os == engine.os.as_ref().unwrap_or(&Os::Linux))
                {
                    *platform
                } else if let Some(platform) =
                    same_arch.iter().find(|platform| platform.os == Os::Linux)
                {
                    // container engine should be fine with linux
                    platform
                } else {
                    let platform = self
                        .toolchain
                        .first()
                        .expect("should be at least one platform");
                    // FIXME: Don't throw away
                    msg_info.warn(
                        format_args!("could not determine what toolchain to use for image, defaulting to `{}`", platform.target),
                    ).ok();
                    platform
                }
            };
            Ok(Image {
                platform: platform.clone(),
                name,
            })
        }
    }
}

impl<T: AsRef<str>> From<T> for PossibleImage {
    fn from(s: T) -> Self {
        PossibleImage {
            reference: s.as_ref().to_owned().into(),
            toolchain: vec![],
        }
    }
}

impl FromStr for PossibleImage {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl std::fmt::Display for PossibleImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reference.get())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(from = "String", untagged)]
pub enum ImageReference {
    /// Partially qualified reference, with or without tag/digest
    Name(String),
    /// Unqualified reference, only a tag or digest
    Identifier(String),
    /// Unqualified reference, only a subtarget
    Subtarget(String),
}

impl ImageReference {
    pub fn get(&self) -> &str {
        match self {
            Self::Name(s) => s,
            Self::Identifier(s) => s,
            Self::Subtarget(s) => s,
        }
    }

    pub fn ensure_qualified(&mut self, target_name: &str) {
        let image_name = match self {
            Self::Name(_) => return,
            Self::Identifier(id) => {
                format!("{CROSS_IMAGE}/{target_name}{id}")
            }
            Self::Subtarget(sub) => {
                format!("{CROSS_IMAGE}/{target_name}:{DEFAULT_IMAGE_VERSION}{sub}")
            }
        };

        *self = Self::Name(image_name);
    }
}

impl From<String> for ImageReference {
    fn from(s: String) -> Self {
        if s.starts_with('-') {
            Self::Subtarget(s)
        } else if s.starts_with(':') || s.starts_with('@') {
            Self::Identifier(s)
        } else {
            Self::Name(s)
        }
    }
}

/// The architecture/platform to use in the image
///
/// <https://github.com/containerd/containerd/blob/release/1.6/platforms/platforms.go#L63>
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct ImagePlatform {
    /// CPU architecture, x86_64, aarch64 etc
    pub architecture: Architecture,
    /// The OS, i.e linux, windows, darwin
    pub os: Os,
    /// The platform variant, i.e v8, v7, v6 etc
    pub variant: Option<String>,
    pub target: TargetTriple,
}

impl ImagePlatform {
    pub const DEFAULT: Self = ImagePlatform::from_const_target(TargetTriple::DEFAULT);
    pub const X86_64_UNKNOWN_LINUX_GNU: Self =
        ImagePlatform::from_const_target(TargetTriple::X86_64UnknownLinuxGnu);
    pub const AARCH64_UNKNOWN_LINUX_GNU: Self =
        ImagePlatform::from_const_target(TargetTriple::Aarch64UnknownLinuxGnu);

    /// Get a representative version of this platform specifier for usage in `--platform`
    ///
    /// Prefer using [`ImagePlatform::specify_platform`] which will supply the flag if needed
    pub fn docker_platform(&self) -> String {
        if let Some(variant) = &self.variant {
            format!("{}/{}/{variant}", self.os, self.architecture)
        } else {
            format!("{}/{}", self.os, self.architecture)
        }
    }

    /// Returns a string that can be used in codegen to represent this platform
    pub fn to_codegen_string(&self) -> Option<&'static str> {
        match self.target {
            TargetTriple::X86_64UnknownLinuxGnu => Some("ImagePlatform::X86_64_UNKNOWN_LINUX_GNU"),
            TargetTriple::Aarch64UnknownLinuxGnu => {
                Some("ImagePlatform::AARCH64_UNKNOWN_LINUX_GNU")
            }
            _ => None,
        }
    }
}

impl Default for ImagePlatform {
    fn default() -> ImagePlatform {
        ImagePlatform::DEFAULT
    }
}

impl TryFrom<String> for ImagePlatform {
    type Error = <Self as std::str::FromStr>::Err;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl Serialize for ImagePlatform {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{}={}", self.docker_platform(), self.target))
    }
}

impl std::str::FromStr for ImagePlatform {
    type Err = eyre::Report;
    // [os/arch[/variant]=]toolchain
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use serde::de::{
            value::{Error as SerdeError, StrDeserializer},
            IntoDeserializer,
        };

        // Try to match the docker platform string first
        match s {
            "linux/amd64" => return Ok(Self::X86_64_UNKNOWN_LINUX_GNU),
            "linux/arm64" | "linux/arm64/v8" => return Ok(Self::AARCH64_UNKNOWN_LINUX_GNU),
            _ => {}
        };

        if let Some((platform, toolchain)) = s.split_once('=') {
            let image_toolchain = toolchain.into();
            let (os, arch, variant) = if let Some((os, rest)) = platform.split_once('/') {
                let os: StrDeserializer<'_, SerdeError> = os.into_deserializer();
                let (arch, variant) = if let Some((arch, variant)) = rest.split_once('/') {
                    let arch: StrDeserializer<'_, SerdeError> = arch.into_deserializer();
                    (arch, Some(variant))
                } else {
                    let arch: StrDeserializer<'_, SerdeError> = rest.into_deserializer();
                    (arch, None)
                };
                (os, arch, variant)
            } else {
                eyre::bail!("invalid platform specified")
            };
            Ok(ImagePlatform {
                architecture: Architecture::deserialize(arch)?,
                os: Os::deserialize(os)?,
                variant: variant.map(ToOwned::to_owned),
                target: image_toolchain,
            })
        } else {
            Ok(ImagePlatform::from_target(s.into())
                .wrap_err_with(|| format!("could not map `{s}` to a platform"))?)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    I386,
    #[serde(alias = "x86_64")]
    Amd64,
    #[serde(alias = "armv7")]
    Arm,
    #[serde(alias = "aarch64")]
    Arm64,
    Mips,
    Mips64,
    Mips64Le,
    MipsLe,
    #[serde(alias = "powerpc64")]
    Ppc64,
    Ppc64Le,
    #[serde(alias = "riscv64gc")]
    Riscv64,
    S390x,
    Wasm,
    #[serde(alias = "loongarch64")]
    LoongArch64,
}

impl Architecture {
    pub fn from_target(target: &TargetTriple) -> Result<Self> {
        let arch = target
            .triple()
            .split_once('-')
            .ok_or_else(|| eyre::eyre!("malformed target"))?
            .0;
        Self::new(arch)
    }

    pub fn new(s: &str) -> Result<Self> {
        use serde::de::IntoDeserializer;

        Self::deserialize(<&str as IntoDeserializer>::into_deserializer(s))
            .wrap_err_with(|| format!("architecture {s} is not supported"))
    }
}

impl std::fmt::Display for Architecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.serialize(f)
    }
}

// Supported Oses are on
// https://rust-lang.github.io/rustup-components-history/aarch64-unknown-linux-gnu.html
// where rust, rustc and cargo is available (e.g rustup toolchain add works)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Android,
    #[serde(alias = "macos")]
    Darwin,
    Freebsd,
    Illumos,
    Linux,
    Netbsd,
    Solaris,
    Windows,
    // Aix
    // Dragonfly
    // Ios
    // Js
    // Openbsd
    // Plan9
}

impl Os {
    pub fn from_target(target: &TargetTriple) -> Result<Self> {
        let mut iter = target.triple().rsplit('-');
        Ok(
            match (
                iter.next().ok_or_else(|| eyre::eyre!("malformed target"))?,
                iter.next().ok_or_else(|| eyre::eyre!("malformed target"))?,
            ) {
                ("darwin", _) => Os::Darwin,
                ("freebsd", _) => Os::Freebsd,
                ("netbsd", _) => Os::Netbsd,
                ("illumos", _) => Os::Illumos,
                ("solaris", _) => Os::Solaris,
                // android targets also set linux, so must occur first
                ("android", _) => Os::Android,
                (_, "linux") => Os::Linux,
                (_, "windows") => Os::Windows,
                (abi, system) => {
                    eyre::bail!("unsupported os in target, abi: {abi:?}, system: {system:?} ")
                }
            },
        )
    }

    pub fn new(s: &str) -> Result<Self> {
        use serde::de::IntoDeserializer;

        Self::deserialize(<&str as IntoDeserializer>::into_deserializer(s))
            .wrap_err_with(|| format!("architecture {s} is not supported"))
    }
}

impl std::fmt::Display for Os {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.serialize(f)
    }
}

impl ImagePlatform {
    pub fn from_target(target: TargetTriple) -> Result<Self> {
        match target {
            target @ TargetTriple::Other(_) => {
                let os = Os::from_target(&target)
                    .wrap_err("could not determine os in target triplet")?;
                let architecture = Architecture::from_target(&target)
                    .wrap_err("could not determine architecture in target triplet")?;
                let variant = match target.triple() {
                    // v7 is default for arm architecture, we still specify it for clarity
                    armv7 if armv7.starts_with("armv7-") => Some("v7".to_owned()),
                    arm if arm.starts_with("arm-") => Some("v6".to_owned()),
                    _ => None,
                };
                Ok(ImagePlatform {
                    architecture,
                    os,
                    variant,
                    target,
                })
            }
            target => Ok(Self::from_const_target(target)),
        }
    }
    #[track_caller]
    pub const fn from_const_target(target: TargetTriple) -> Self {
        match target {
            TargetTriple::Other(_) => {
                unimplemented!()
            }
            TargetTriple::X86_64AppleDarwin => ImagePlatform {
                architecture: Architecture::Amd64,
                os: Os::Darwin,
                variant: None,
                target,
            },
            TargetTriple::Aarch64AppleDarwin => ImagePlatform {
                architecture: Architecture::Arm64,
                os: Os::Linux,
                variant: None,
                target,
            },
            TargetTriple::X86_64UnknownLinuxGnu => ImagePlatform {
                architecture: Architecture::Amd64,
                os: Os::Linux,
                variant: None,
                target,
            },
            TargetTriple::Aarch64UnknownLinuxGnu => ImagePlatform {
                architecture: Architecture::Arm64,
                os: Os::Linux,
                variant: None,
                target,
            },
            TargetTriple::X86_64UnknownLinuxMusl => ImagePlatform {
                architecture: Architecture::Amd64,
                os: Os::Linux,
                variant: None,
                target,
            },
            TargetTriple::Aarch64UnknownLinuxMusl => ImagePlatform {
                architecture: Architecture::Arm64,
                os: Os::Linux,
                variant: None,
                target,
            },
            TargetTriple::X86_64PcWindowsMsvc => ImagePlatform {
                architecture: Architecture::Amd64,
                os: Os::Windows,
                variant: None,
                target,
            },
        }
    }

    pub fn specify_platform(&self, engine: &Engine, cmd: &mut std::process::Command) {
        if self.variant.is_none()
            && Some(&self.architecture) == engine.arch.as_ref()
            && Some(&self.os) == engine.os.as_ref()
        {
        } else {
            cmd.args(["--platform", &self.docker_platform()]);
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    macro_rules! t {
        ($t:literal) => {
            TargetTriple::from($t)
        };
    }

    macro_rules! arch {
        ($t:literal) => {
            Architecture::from_target(&TargetTriple::from($t))
        };
    }

    #[test]
    fn architecture_from_target() -> Result<()> {
        assert_eq!(arch!("x86_64-apple-darwin")?, Architecture::Amd64);
        assert_eq!(arch!("arm-unknown-linux-gnueabihf")?, Architecture::Arm);
        assert_eq!(arch!("armv7-unknown-linux-gnueabihf")?, Architecture::Arm);
        assert_eq!(arch!("aarch64-unknown-linux-gnu")?, Architecture::Arm64);
        assert_eq!(arch!("aarch64-unknown-freebsd")?, Architecture::Arm64);
        assert_eq!(
            arch!("loongarch64-unknown-linux-gnu")?,
            Architecture::LoongArch64
        );
        assert_eq!(arch!("mips-unknown-linux-gnu")?, Architecture::Mips);
        assert_eq!(
            arch!("mips64-unknown-linux-gnuabi64")?,
            Architecture::Mips64
        );
        assert_eq!(
            arch!("mips64le-unknown-linux-gnuabi64")?,
            Architecture::Mips64Le
        );
        assert_eq!(arch!("mipsle-unknown-linux-gnu")?, Architecture::MipsLe);
        Ok(())
    }

    #[test]
    fn os_from_target() -> Result<()> {
        assert_eq!(Os::from_target(&t!("x86_64-apple-darwin"))?, Os::Darwin);
        assert_eq!(Os::from_target(&t!("x86_64-unknown-freebsd"))?, Os::Freebsd);
        assert_eq!(
            Os::from_target(&t!("aarch64-unknown-freebsd"))?,
            Os::Freebsd
        );
        assert_eq!(
            Os::from_target(&t!("loongarch64-unknown-linux-gnu"))?,
            Os::Linux
        );
        assert_eq!(Os::from_target(&t!("x86_64-unknown-netbsd"))?, Os::Netbsd);
        assert_eq!(Os::from_target(&t!("sparcv9-sun-solaris"))?, Os::Solaris);
        assert_eq!(Os::from_target(&t!("sparcv9-sun-illumos"))?, Os::Illumos);
        assert_eq!(Os::from_target(&t!("aarch64-linux-android"))?, Os::Android);
        assert_eq!(Os::from_target(&t!("x86_64-unknown-linux-gnu"))?, Os::Linux);
        assert_eq!(Os::from_target(&t!("x86_64-pc-windows-msvc"))?, Os::Windows);
        Ok(())
    }

    #[test]
    fn image_platform_from_docker_platform_str() -> Result<()> {
        assert_eq!(
            "linux/amd64".parse::<ImagePlatform>()?,
            ImagePlatform::X86_64_UNKNOWN_LINUX_GNU
        );

        assert_eq!(
            "linux/arm64".parse::<ImagePlatform>()?,
            ImagePlatform::AARCH64_UNKNOWN_LINUX_GNU
        );

        assert_eq!(
            "linux/arm64/v8".parse::<ImagePlatform>()?,
            ImagePlatform::AARCH64_UNKNOWN_LINUX_GNU
        );

        Ok(())
    }
}
