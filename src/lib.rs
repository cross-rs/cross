//! [![crates.io](https://img.shields.io/crates/v/cross.svg)](https://crates.io/crates/cross)
//! [![crates.io](https://img.shields.io/crates/d/cross.svg)](https://crates.io/crates/cross)
//! [![Matrix](https://img.shields.io/matrix/cross-rs:matrix.org)](https://matrix.to/#/#cross-rs:matrix.org)
//!
//! # `cross`
//!
//! <p style="background:rgba(148,192,255,0.1);padding:0.5em;border-radius:0.2em">
//! <strong>⚠️ Warning:</strong> The cross library is for internal
//! use only: only the command-line interface is stable. The library
//! may change at any point for any reason. For documentation on the
//! CLI, please see the repository <a href="https://github.com/cross-rs/cross">README</a>
//! or the <a href="https://github.com/cross-rs/cross/wiki">wiki</a>.
//! </p>

#![deny(missing_debug_implementations, rust_2018_idioms)]
#![warn(
    clippy::explicit_into_iter_loop,
    clippy::explicit_iter_loop,
    clippy::implicit_clone,
    clippy::inefficient_to_string,
    clippy::map_err_ignore,
    clippy::map_unwrap_or,
    clippy::ref_binding_to_reference,
    clippy::semicolon_if_nothing_returned,
    clippy::str_to_string,
    clippy::string_to_string,
    // needs clippy 1.61 clippy::unwrap_used
)]

#[cfg(test)]
mod tests;

pub mod cargo;
pub mod cli;
pub mod config;
mod cross_toml;
pub mod docker;
pub mod errors;
mod extensions;
pub mod file;
mod id;
mod interpreter;
pub mod rustc;
pub mod rustup;
pub mod shell;
pub mod temp;

use std::env;
use std::path::PathBuf;
use std::process::ExitStatus;

use cli::Args;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::{Help, SectionExt};
use config::Config;
use rustc::{QualifiedToolchain, Toolchain};
use rustc_version::Channel;
use serde::{Deserialize, Serialize, Serializer};

pub use self::cargo::{cargo_command, cargo_metadata_with_args, CargoMetadata, Subcommand};
pub use self::cross_toml::CrossToml;
use self::errors::Context;
use self::shell::{MessageInfo, Verbosity};

pub use self::errors::{install_panic_hook, install_termination_hook, Result};
pub use self::extensions::{CommandExt, OutputExt};
pub use self::file::{pretty_path, ToUtf8};
pub use self::rustc::{TargetList, VersionMetaExt};

pub const CROSS_LABEL_DOMAIN: &str = "org.cross-rs";

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Hash, Serialize)]
#[serde(from = "&str")]
#[serde(rename_all = "snake_case")]
pub enum TargetTriple {
    Other(String),

    // OSX
    X86_64AppleDarwin,
    // Support Apple Silicon, as developers are starting to use it as development workstation.
    Aarch64AppleDarwin,

    // Linux
    X86_64UnknownLinuxGnu,
    // Linux Aarch64 is become more popular in CI pipelines (e.g. to AWS Graviton based systems)
    Aarch64UnknownLinuxGnu,
    // (Alpine) Linux (musl) often use in CI pipelines to cross compile rust projects to different
    // targets (e.g. in GitLab CI pipelines).
    X86_64UnknownLinuxMusl,
    // (Alpine) Linux (musl) often use in CI pipelines to cross compile rust projects to different
    // targets (e.g. in GitLab CI pipelines). Now, that AWS Graviton based systems are gaining
    // attraction CI pipelines might run on (Alpine) Linux Aarch64.
    Aarch64UnknownLinuxMusl,

    // Windows MSVC
    X86_64PcWindowsMsvc,
}

impl TargetTriple {
    pub const DEFAULT: Self = Self::X86_64UnknownLinuxGnu;
    /// Returns the architecture name according to `dpkg` naming convention
    ///
    /// # Notes
    ///
    /// Some of these make no sense to use in our standard images
    pub fn deb_arch(&self) -> Option<&'static str> {
        match self.triple() {
            "aarch64-unknown-linux-gnu" => Some("arm64"),
            "aarch64-unknown-linux-musl" => Some("musl-linux-arm64"),
            "aarch64-linux-android" => None,
            "x86_64-unknown-linux-gnu" => Some("amd64"),
            "x86_64-apple-darwin" => Some("darwin-amd64"),
            "x86_64-unknown-linux-musl" => Some("musl-linux-amd64"),

            "x86_64-pc-windows-msvc" => None,
            "arm-unknown-linux-gnueabi" => Some("armel"),
            "arm-unknown-linux-gnueabihf" => Some("armhf"),
            "armv7-unknown-linux-gnueabi" => Some("armel"),
            "armv7-unknown-linux-gnueabihf" => Some("armhf"),
            "thumbv7neon-unknown-linux-gnueabihf" => Some("armhf"),
            "i586-unknown-linux-gnu" => Some("i386"),
            "i686-unknown-linux-gnu" => Some("i386"),
            "mips-unknown-linux-gnu" => Some("mips"),
            "mipsel-unknown-linux-gnu" => Some("mipsel"),
            "mips64-unknown-linux-gnuabi64" => Some("mips64"),
            "mips64el-unknown-linux-gnuabi64" => Some("mips64el"),
            "mips64-unknown-linux-muslabi64" => Some("musl-linux-mips64"),
            "mips64el-unknown-linux-muslabi64" => Some("musl-linux-mips64el"),
            "powerpc-unknown-linux-gnu" => Some("powerpc"),
            "powerpc64-unknown-linux-gnu" => Some("ppc64"),
            "powerpc64le-unknown-linux-gnu" => Some("ppc64el"),
            "riscv64gc-unknown-linux-gnu" => Some("riscv64"),
            "s390x-unknown-linux-gnu" => Some("s390x"),
            "sparc64-unknown-linux-gnu" => Some("sparc64"),
            "arm-unknown-linux-musleabihf" => Some("musl-linux-armhf"),
            "arm-unknown-linux-musleabi" => Some("musl-linux-arm"),
            "armv5te-unknown-linux-gnueabi" => None,
            "armv5te-unknown-linux-musleabi" => None,
            "armv7-unknown-linux-musleabi" => Some("musl-linux-arm"),
            "armv7-unknown-linux-musleabihf" => Some("musl-linux-armhf"),
            "i586-unknown-linux-musl" => Some("musl-linux-i386"),
            "i686-unknown-linux-musl" => Some("musl-linux-i386"),
            "mips-unknown-linux-musl" => Some("musl-linux-mips"),
            "mipsel-unknown-linux-musl" => Some("musl-linux-mipsel"),
            "arm-linux-androideabi" => None,
            "armv7-linux-androideabi" => None,
            "thumbv7neon-linux-androideabi" => None,
            "i686-linux-android" => None,
            "x86_64-linux-android" => None,
            "x86_64-pc-windows-gnu" => None,
            "i686-pc-windows-gnu" => None,
            "asmjs-unknown-emscripten" => None,
            "wasm32-unknown-emscripten" => None,
            "x86_64-unknown-dragonfly" => Some("dragonflybsd-amd64"),
            "i686-unknown-freebsd" => Some("freebsd-i386"),
            "x86_64-unknown-freebsd" => Some("freebsd-amd64"),
            "x86_64-unknown-netbsd" => Some("netbsd-amd64"),
            "sparcv9-sun-solaris" => Some("solaris-sparc"),
            "x86_64-sun-solaris" => Some("solaris-amd64"),
            "thumbv6m-none-eabi" => Some("arm"),
            "thumbv7em-none-eabi" => Some("arm"),
            "thumbv7em-none-eabihf" => Some("armhf"),
            "thumbv7m-none-eabi" => Some("arm"),
            _ => None,
        }
    }

    /// Checks if this `(host, target)` pair is supported by `cross`
    ///
    /// `target == None` means `target == host`
    fn is_supported(&self, target: Option<&Target>) -> bool {
        match std::env::var("CROSS_COMPATIBILITY_VERSION")
            .as_ref()
            .map(|v| v.as_str())
        {
            // Old behavior (up to cross version 0.2.1) can be activated on demand using environment
            // variable `CROSS_COMPATIBILITY_VERSION`.
            Ok("0.2.1") => match self {
                TargetTriple::X86_64AppleDarwin | TargetTriple::Aarch64AppleDarwin => {
                    target.map_or(false, |t| t.needs_docker())
                }
                TargetTriple::X86_64UnknownLinuxGnu
                | TargetTriple::Aarch64UnknownLinuxGnu
                | TargetTriple::X86_64UnknownLinuxMusl
                | TargetTriple::Aarch64UnknownLinuxMusl => {
                    target.map_or(true, |t| t.needs_docker())
                }
                TargetTriple::X86_64PcWindowsMsvc => target.map_or(false, |t| {
                    t.triple() != TargetTriple::X86_64PcWindowsMsvc.triple() && t.needs_docker()
                }),
                TargetTriple::Other(_) => false,
            },
            // New behaviour, if a target is provided (--target ...) then always run with docker
            // image unless the target explicitly opts-out (i.e. unless needs_docker() returns false).
            // If no target is provided run natively (on host) using cargo.
            //
            // This not only simplifies the logic, it also enables forward-compatibility without
            // having to change cross every time someone comes up with the need for a new host/target
            // combination. It's totally fine to call cross with `--target=$host_triple`, for
            // example to test custom docker images. Cross should not try to recognize if host and
            // target are equal, it's a user decision and if user wants to bypass cross he can call
            // cargo directly or omit the `--target` option.
            _ => target.map_or(false, |t| t.needs_docker()),
        }
    }

    /// Returns the [`Target`] as target triple string
    pub fn triple(&self) -> &str {
        match self {
            TargetTriple::X86_64AppleDarwin => "x86_64-apple-darwin",
            TargetTriple::Aarch64AppleDarwin => "aarch64-apple-darwin",
            TargetTriple::X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            TargetTriple::Aarch64UnknownLinuxGnu => "aarch64-unknown-linux-gnu",
            TargetTriple::X86_64UnknownLinuxMusl => "x86_64-unknown-linux-musl",
            TargetTriple::Aarch64UnknownLinuxMusl => "aarch64-unknown-linux-musl",
            TargetTriple::X86_64PcWindowsMsvc => "x86_64-pc-windows-msvc",
            TargetTriple::Other(s) => s.as_str(),
        }
    }
}

impl<'a> From<&'a str> for TargetTriple {
    fn from(s: &str) -> TargetTriple {
        match s {
            "x86_64-apple-darwin" => TargetTriple::X86_64AppleDarwin,
            "x86_64-unknown-linux-gnu" => TargetTriple::X86_64UnknownLinuxGnu,
            "x86_64-unknown-linux-musl" => TargetTriple::X86_64UnknownLinuxMusl,
            "x86_64-pc-windows-msvc" => TargetTriple::X86_64PcWindowsMsvc,
            "aarch64-apple-darwin" => TargetTriple::Aarch64AppleDarwin,
            "aarch64-unknown-linux-gnu" => TargetTriple::Aarch64UnknownLinuxGnu,
            "aarch64-unknown-linux-musl" => TargetTriple::Aarch64UnknownLinuxMusl,
            s => TargetTriple::Other(s.to_owned()),
        }
    }
}

impl Default for TargetTriple {
    fn default() -> TargetTriple {
        TargetTriple::DEFAULT
    }
}

impl std::str::FromStr for TargetTriple {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl std::fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.triple())
    }
}

impl From<String> for TargetTriple {
    fn from(s: String) -> TargetTriple {
        s.as_str().into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(from = "String")]
pub enum Target {
    BuiltIn { triple: TargetTriple },
    Custom { triple: TargetTriple },
}

impl Target {
    pub const DEFAULT: Self = Self::BuiltIn {
        triple: TargetTriple::DEFAULT,
    };

    fn new_built_in(triple: &str) -> Self {
        Target::BuiltIn {
            triple: triple.into(),
        }
    }

    fn new_custom(triple: &str) -> Self {
        Target::Custom {
            triple: triple.into(),
        }
    }

    pub fn triple(&self) -> &str {
        match *self {
            Target::BuiltIn { ref triple } => triple.triple(),
            Target::Custom { ref triple } => triple.triple(),
        }
    }

    pub fn target(&self) -> &TargetTriple {
        match *self {
            Target::BuiltIn { ref triple } => triple,
            Target::Custom { ref triple } => triple,
        }
    }

    fn is_apple(&self) -> bool {
        self.triple().contains("apple")
    }

    fn is_bare_metal(&self) -> bool {
        self.triple().contains("thumb")
    }

    fn is_builtin(&self) -> bool {
        match *self {
            Target::BuiltIn { .. } => true,
            Target::Custom { .. } => false,
        }
    }

    fn is_bsd(&self) -> bool {
        self.triple().contains("bsd") || self.triple().contains("dragonfly")
    }

    fn is_solaris(&self) -> bool {
        self.triple().contains("solaris")
    }

    fn is_illumos(&self) -> bool {
        self.triple().contains("illumos")
    }

    fn is_android(&self) -> bool {
        self.triple().contains("android")
    }

    fn is_emscripten(&self) -> bool {
        self.triple().contains("emscripten")
    }

    fn is_linux(&self) -> bool {
        self.triple().contains("linux") && !self.is_android()
    }

    fn is_windows(&self) -> bool {
        self.triple().contains("windows")
    }

    fn needs_docker(&self) -> bool {
        self.is_linux()
            || self.is_android()
            || self.is_bare_metal()
            || self.is_bsd()
            || self.is_solaris()
            || self.is_illumos()
            || !self.is_builtin()
            || self.is_windows()
            || self.is_emscripten()
            || self.is_apple()
    }

    fn needs_interpreter(&self) -> bool {
        let native = self.triple().starts_with("x86_64")
            || self.triple().starts_with("i586")
            || self.triple().starts_with("i686");

        !native && (self.is_linux() || self.is_windows() || self.is_bare_metal())
    }

    fn needs_docker_seccomp(&self) -> bool {
        let arch_32bit = self.triple().starts_with("arm")
            || self.triple().starts_with("thumb")
            || self.triple().starts_with("i586")
            || self.triple().starts_with("i686");

        arch_32bit && self.is_android()
    }
}

impl Default for Target {
    fn default() -> Target {
        Target::DEFAULT
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.triple())
    }
}

impl Target {
    pub fn from(triple: &str, target_list: &TargetList) -> Target {
        if target_list.contains(triple) {
            Target::new_built_in(triple)
        } else {
            Target::new_custom(triple)
        }
    }
}

impl From<TargetTriple> for Target {
    fn from(host: TargetTriple) -> Target {
        match host {
            TargetTriple::X86_64UnknownLinuxGnu => Target::new_built_in("x86_64-unknown-linux-gnu"),
            TargetTriple::X86_64UnknownLinuxMusl => {
                Target::new_built_in("x86_64-unknown-linux-musl")
            }
            TargetTriple::X86_64AppleDarwin => Target::new_built_in("x86_64-apple-darwin"),
            TargetTriple::X86_64PcWindowsMsvc => Target::new_built_in("x86_64-pc-windows-msvc"),
            TargetTriple::Aarch64AppleDarwin => Target::new_built_in("aarch64-apple-darwin"),
            TargetTriple::Aarch64UnknownLinuxGnu => {
                Target::new_built_in("aarch64-unknown-linux-gnu")
            }
            TargetTriple::Aarch64UnknownLinuxMusl => {
                Target::new_built_in("aarch64-unknown-linux-musl")
            }
            TargetTriple::Other(s) => Target::from(
                s.as_str(),
                &rustc::target_list(&mut Verbosity::Quiet.into())
                    .expect("should be able to query rustc"),
            ),
        }
    }
}

impl From<String> for Target {
    fn from(target_str: String) -> Target {
        let target_host: TargetTriple = target_str.as_str().into();
        target_host.into()
    }
}

impl Serialize for Target {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.triple())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoVariant {
    Cargo,
    Xargo,
    Zig,
}

impl CargoVariant {
    pub fn create(uses_zig: bool, uses_xargo: bool) -> Result<CargoVariant> {
        match (uses_zig, uses_xargo) {
            (true, true) => eyre::bail!("cannot use both zig and xargo"),
            (true, false) => Ok(CargoVariant::Zig),
            (false, true) => Ok(CargoVariant::Xargo),
            (false, false) => Ok(CargoVariant::Cargo),
        }
    }

    pub fn to_str(self) -> &'static str {
        match self {
            CargoVariant::Cargo => "cargo",
            CargoVariant::Xargo => "xargo",
            CargoVariant::Zig => "cargo-zigbuild",
        }
    }

    pub fn uses_xargo(self) -> bool {
        self == CargoVariant::Xargo
    }

    pub fn uses_zig(self) -> bool {
        self == CargoVariant::Zig
    }
}

fn warn_on_failure(
    target: &Target,
    toolchain: &QualifiedToolchain,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    let rust_std = format!("rust-std-{target}");
    if target.is_builtin() {
        let component = rustup::check_component(&rust_std, toolchain, msg_info)?;
        if component.is_not_available() {
            msg_info.warn(format_args!("rust-std is not available for {target}"))?;
            msg_info.note(
                format_args!(
                    r#"you may need to build components for the target via `-Z build-std=<components>` or in your cross configuration specify `target.{target}.build-std`
              the available components are core, std, alloc, and proc_macro"#
                ),
            )?;
        }
    }
    Ok(())
}

fn add_libc_version(triple: &str, zig_version: Option<&str>) -> String {
    match zig_version {
        Some(libc) => format!("{triple}.{libc}"),
        None => triple.to_owned(),
    }
}

pub fn run(
    args: Args,
    target_list: TargetList,
    msg_info: &mut MessageInfo,
) -> Result<Option<ExitStatus>> {
    let host_version_meta = rustc::version_meta()?;

    let cwd = std::env::current_dir()?;
    if let Some(metadata) = cargo_metadata_with_args(None, Some(&args), msg_info)? {
        let host = host_version_meta.host();
        let toml = toml(&metadata, msg_info)?;
        let config = Config::new(toml);
        let target = args
            .target
            .or_else(|| config.target(&target_list))
            .unwrap_or_else(|| Target::from(host.triple(), &target_list));
        config.confusable_target(&target, msg_info)?;

        let uses_zig = config.zig(&target).unwrap_or(false);
        let zig_version = config.zig_version(&target)?;
        // Get the image we're supposed to base all our next actions on.
        // The image we actually run in might get changed with
        // `target.{{TARGET}}.dockerfile` or `target.{{TARGET}}.pre-build`
        let image = match docker::get_image(&config, &target, uses_zig) {
            Ok(i) => i,
            Err(err) => {
                msg_info.warn(err)?;

                return Ok(None);
            }
        };

        // Grab the current toolchain, this might be the one we mount in the image later
        let default_toolchain = QualifiedToolchain::default(&config, msg_info)?;

        // `cross +channel`, where channel can be `+channel[-YYYY-MM-DD]`
        let mut toolchain = if let Some(channel) = args.channel {
            let picked_toolchain: Toolchain = channel.parse()?;

            if let Some(picked_host) = &picked_toolchain.host {
                return Err(eyre::eyre!("the specified toolchain `{picked_toolchain}` can't be used"))
                    .with_suggestion(|| {
                        format!(
                            "try `cross +{}` instead",
                            Toolchain {
                                host: None,
                                ..picked_toolchain
                            }
                        )
                    }).with_section(|| format!(
r#"Overriding the toolchain in cross is only possible in CLI by specifying a channel and optional date: `+channel[-YYYY-MM-DD]`.
To override the toolchain mounted in the image, set `target.{}.image.toolchain = "{picked_host}"`"#, target).header("Note:".bright_cyan()));
            }

            default_toolchain.with_picked(picked_toolchain)?
        } else {
            default_toolchain
        };

        let is_remote = docker::Engine::is_remote();
        let engine = docker::Engine::new(None, Some(is_remote), msg_info)?;

        let image = image.to_definite_with(&engine, msg_info);

        toolchain.replace_host(&image.platform);
        let maybe_warn = matches!(toolchain.channel.as_str(), "stable" | "beta" | "nightly");

        if image.platform.target.is_supported(Some(&target)) {
            if image.platform.architecture != toolchain.host().architecture {
                msg_info.warn(format_args!(
                    "toolchain `{toolchain}` may not run on image `{image}`"
                ))?;
            }
            // set the sysroot explicitly to the toolchain
            let mut is_nightly = toolchain.channel.contains("nightly");

            if !toolchain.is_custom
                && !rustup::installed_toolchains(msg_info)?
                    .into_iter()
                    .any(|t| t == toolchain.to_string())
            {
                rustup::install_toolchain(&toolchain, msg_info)?;
            }
            let available_targets = if !toolchain.is_custom {
                rustup::available_targets(&toolchain.full, msg_info)?
            } else {
                rustup::AvailableTargets {
                    default: String::new(),
                    installed: vec![],
                    not_installed: vec![],
                }
            };

            let mut rustc_version = None;
            if let Some((version, channel, commit)) = toolchain.rustc_version()? {
                if maybe_warn && toolchain.date.is_none() {
                    warn_host_version_mismatch(
                        &host_version_meta,
                        &toolchain,
                        &version,
                        &commit,
                        msg_info,
                    )?;
                }
                is_nightly = channel == Channel::Nightly;
                rustc_version = Some(version);
            }

            let uses_build_std = config.build_std(&target).unwrap_or(false);
            let uses_xargo =
                !uses_build_std && config.xargo(&target).unwrap_or(!target.is_builtin());
            let cargo_variant = CargoVariant::create(uses_zig, uses_xargo)?;
            if !toolchain.is_custom {
                // build-std overrides xargo, but only use it if it's a built-in
                // tool but not an available target or doesn't have rust-std.

                if !is_nightly && uses_build_std {
                    eyre::bail!(
                        "no rust-std component available for {}: must use nightly",
                        target.triple()
                    );
                }

                if !uses_xargo
                    && !uses_build_std
                    && !available_targets.is_installed(&target)
                    && available_targets.contains(&target)
                {
                    rustup::install(&target, &toolchain, msg_info)?;
                } else if !rustup::component_is_installed("rust-src", &toolchain, msg_info)? {
                    rustup::install_component("rust-src", &toolchain, msg_info)?;
                }
                if args.subcommand.map_or(false, |sc| sc == Subcommand::Clippy)
                    && !rustup::component_is_installed("clippy", &toolchain, msg_info)?
                {
                    rustup::install_component("clippy", &toolchain, msg_info)?;
                }
            }

            let needs_interpreter = args.subcommand.map_or(false, |sc| sc.needs_interpreter());

            let add_libc = |triple: &str| add_libc_version(triple, zig_version.as_deref());
            let mut filtered_args = if args
                .subcommand
                .map_or(false, |s| !s.needs_target_in_command())
            {
                let mut filtered_args = Vec::new();
                let mut args_iter = args.cargo_args.clone().into_iter();
                while let Some(arg) = args_iter.next() {
                    if arg == "--target" {
                        args_iter.next();
                    } else if arg.starts_with("--target=") {
                        // NOOP
                    } else {
                        filtered_args.push(arg);
                    }
                }
                filtered_args
            // Make sure --target is present
            } else if !args.cargo_args.iter().any(|a| a.starts_with("--target")) {
                let mut args_with_target = args.cargo_args.clone();
                args_with_target.push("--target".to_owned());
                args_with_target.push(add_libc(target.triple()));
                args_with_target
            } else if zig_version.is_some() {
                let mut filtered_args = Vec::new();
                let mut args_iter = args.cargo_args.clone().into_iter();
                while let Some(arg) = args_iter.next() {
                    if arg == "--target" {
                        filtered_args.push("--target".to_owned());
                        if let Some(triple) = args_iter.next() {
                            filtered_args.push(add_libc(&triple));
                        }
                    } else if let Some(stripped) = arg.strip_prefix("--target=") {
                        filtered_args.push(format!("--target={}", add_libc(stripped)));
                    } else {
                        filtered_args.push(arg);
                    }
                }
                filtered_args
            } else {
                args.cargo_args.clone()
            };

            let is_test = args.subcommand.map_or(false, |sc| sc == Subcommand::Test);
            if is_test && config.doctests().unwrap_or_default() && is_nightly {
                filtered_args.push("-Zdoctest-xcompile".to_owned());
            }
            if uses_build_std {
                filtered_args.push("-Zbuild-std".to_owned());
            }
            filtered_args.extend(args.rest_args.iter().cloned());

            let needs_docker = args
                .subcommand
                .map_or(false, |sc| sc.needs_docker(is_remote));
            if target.needs_docker() && needs_docker {
                if host_version_meta.needs_interpreter()
                    && needs_interpreter
                    && target.needs_interpreter()
                    && !interpreter::is_registered(&target)?
                {
                    engine.register_binfmt(&target, msg_info)?;
                }

                let paths = docker::DockerPaths::create(
                    &engine,
                    metadata,
                    cwd,
                    toolchain.clone(),
                    msg_info,
                )?;
                let options = docker::DockerOptions::new(
                    engine,
                    target.clone(),
                    config,
                    image,
                    cargo_variant,
                    rustc_version,
                );
                let status = docker::run(options, paths, &filtered_args, msg_info)
                    .wrap_err("could not run container")?;
                let needs_host = args.subcommand.map_or(false, |sc| sc.needs_host(is_remote));
                if !status.success() {
                    warn_on_failure(&target, &toolchain, msg_info)?;
                }
                if !(status.success() && needs_host) {
                    return Ok(Some(status));
                }
            }
        }
    }
    Ok(None)
}

pub const fn version() -> &'static str {
    concat!("cross ", env!("CARGO_PKG_VERSION"), crate::commit_info!())
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum VersionMatch {
    Same,
    OlderTarget,
    NewerTarget,
    Different,
}

pub(crate) fn warn_host_version_mismatch(
    host_version_meta: &rustc_version::VersionMeta,
    toolchain: &QualifiedToolchain,
    rustc_version: &rustc_version::Version,
    rustc_commit: &str,
    msg_info: &mut MessageInfo,
) -> Result<VersionMatch> {
    let host_commit = host_version_meta.short_version_string.splitn(3, ' ').nth(2);
    let rustc_commit_date = rustc_commit
        .split_once(' ')
        .and_then(|x| x.1.strip_suffix(')'));

    if rustc_version != &host_version_meta.semver || (Some(rustc_commit) != host_commit) {
        let versions = rustc_version.cmp(&host_version_meta.semver);
        let dates = rustc_commit_date.cmp(&host_version_meta.commit_date.as_deref());

        let rustc_warning = format!(
            "rustc `{rustc_version} {rustc_commit}` for the target. Current active rustc on the host is `{}`",
            host_version_meta.short_version_string
        );
        if versions.is_lt() || (versions.is_eq() && dates.is_lt()) {
            if cfg!(not(test)) {
                msg_info.warn(format_args!("using older {rustc_warning}.\n > Update with `rustup update --force-non-host {toolchain}`"))?;
            }
            return Ok(VersionMatch::OlderTarget);
        } else if versions.is_gt() || (versions.is_eq() && dates.is_gt()) {
            if cfg!(not(test)) {
                msg_info.warn(format_args!(
                    "using newer {rustc_warning}.\n > Update with `rustup update`"
                ))?;
            }
            return Ok(VersionMatch::NewerTarget);
        } else {
            if cfg!(not(test)) {
                msg_info.warn(format_args!("using {rustc_warning}."))?;
            }
            return Ok(VersionMatch::Different);
        }
    }
    Ok(VersionMatch::Same)
}

pub const fn commit_info() -> &'static str {
    commit_info!()
}

#[macro_export]
macro_rules! commit_info {
    () => {
        include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt"))
    };
}

/// Obtains the [`CrossToml`] from one of the possible locations
///
/// These locations are checked in the following order:
/// 1. If the `CROSS_CONFIG` variable is set, it tries to read the config from its value
/// 2. Otherwise, the `Cross.toml` in the project root is used
/// 3. Package metadata in the Cargo.toml
///
/// The values from `CROSS_CONFIG` or `Cross.toml` are concatenated with the package
/// metadata in `Cargo.toml`, with `Cross.toml` having the highest priority.
fn toml(metadata: &CargoMetadata, msg_info: &mut MessageInfo) -> Result<Option<CrossToml>> {
    let root = &metadata.workspace_root;
    let cross_config_path = match env::var("CROSS_CONFIG") {
        Ok(var) => PathBuf::from(var),
        Err(_) => root.join("Cross.toml"),
    };

    // Attempts to read the cross config from the Cargo.toml
    let cargo_toml_str =
        file::read(root.join("Cargo.toml")).wrap_err("failed to read Cargo.toml")?;

    if cross_config_path.exists() {
        let cross_toml_str = file::read(&cross_config_path)
            .wrap_err_with(|| format!("could not read file `{cross_config_path:?}`"))?;

        let (config, _) = CrossToml::parse(&cargo_toml_str, &cross_toml_str, msg_info)
            .wrap_err_with(|| format!("failed to parse file `{cross_config_path:?}` as TOML",))?;

        Ok(Some(config))
    } else {
        // Checks if there is a lowercase version of this file
        if root.join("cross.toml").exists() {
            msg_info.warn("There's a file named cross.toml, instead of Cross.toml. You may want to rename it, or it won't be considered.")?;
        }

        if let Some((cfg, _)) = CrossToml::parse_from_cargo(&cargo_toml_str, msg_info)? {
            Ok(Some(cfg))
        } else {
            Ok(None)
        }
    }
}
