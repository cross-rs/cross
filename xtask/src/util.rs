use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use cross::shell::MessageInfo;
use cross::{docker, CommandExt, ToUtf8};

use once_cell::sync::{Lazy, OnceCell};
use serde::Deserialize;

static WORKSPACE: OnceCell<PathBuf> = OnceCell::new();

/// Returns the cargo workspace for the manifest
pub fn get_cargo_workspace() -> &'static Path {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    WORKSPACE.get_or_init(|| {
        cross::cargo_metadata_with_args(
            Some(manifest_dir.as_ref()),
            None,
            &mut MessageInfo::create(2, false, None).expect("should not fail"),
        )
        .unwrap()
        .unwrap()
        .workspace_root
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct CiTarget {
    /// The name of the target. This can either be a target triple, or if the image is "special", the name of the special thing it does.
    pub target: String,
    #[serde(default)]
    pub special: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// The runner to use in CI, see https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions#choosing-github-hosted-runners
    ///
    /// if this is not equal to `ubuntu-latest`, no docker image will be built unless it's been special cased.
    pub os: String,
    /// if `true` test more extensive cargo support, including tests and running binaries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<bool>,
    /// if `true` publish the generated binaries for cross
    #[serde(default)]
    pub deploy: Option<bool>,
    /// the platform to build this image for, defaults to `["linux/amd64"]`, takes multiple
    #[serde(skip_serializing_if = "Option::is_none")]
    platforms: Option<Vec<String>>,
    /// if `true` signal that this target requires `-Zbuild-std`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_std: Option<bool>,
    /// test the cpp compiler
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpp: Option<bool>,
    /// test dylib support, requires `run = true`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dylib: Option<bool>,
    /// qemu runners that can be used with this target, space separated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runners: Option<String>,
    /// if `true` test no std support as if std does exists. If `false` build https://github.com/rust-lang/compiler-builtins
    #[serde(skip_serializing_if = "Option::is_none")]
    pub std: Option<bool>,
    #[serde(skip_serializing_if = "is_false", default)]
    pub disabled: bool,
}

pub fn is_false(b: &bool) -> bool {
    !*b
}

impl CiTarget {
    pub fn has_test(&self, target: &str) -> bool {
        // bare-metal targets don't have unittests right now
        self.run.unwrap_or_default() && !target.contains("-none-")
    }

    pub fn to_image_target(&self) -> crate::ImageTarget {
        crate::ImageTarget {
            name: self.target.clone(),
            sub: self.sub.clone(),
        }
    }

    pub fn builds_image(&self) -> bool {
        self.os == "ubuntu-latest"
    }

    pub fn platforms(&self) -> &[String] {
        self.platforms.as_ref().unwrap_or(&DEFAULT_PLATFORMS_STRING)
    }
}

/// Default platforms to build images with
///
///  if this is changed, make sure to update documentation on [CiTarget::platforms]
pub static DEFAULT_PLATFORMS: &[cross::docker::ImagePlatform] =
    &[cross::docker::ImagePlatform::DEFAULT];

pub static DEFAULT_PLATFORMS_STRING: Lazy<Vec<String>> = Lazy::new(|| {
    DEFAULT_PLATFORMS
        .iter()
        .map(|p| p.target.to_string())
        .collect()
});

static MATRIX: OnceCell<Vec<CiTarget>> = OnceCell::new();

pub fn get_matrix() -> &'static Vec<CiTarget> {
    #[derive(Deserialize)]
    struct Targets {
        target: Vec<CiTarget>,
    }
    MATRIX
        .get_or_try_init::<_, eyre::Report>(|| {
            let targets: Targets = toml::from_str(std::str::from_utf8(&std::fs::read(
                get_cargo_workspace().join("targets.toml"),
            )?)?)?;
            Ok(targets.target)
        })
        .unwrap()
}

pub fn with_section_reports(
    origin: eyre::Report,
    iter: impl IntoIterator<Item = eyre::Report>,
) -> eyre::Report {
    use color_eyre::{Section as _, SectionExt as _};
    iter.into_iter().fold(origin, |report, e| {
        report.section(format!("{e:?}").header("Error:"))
    })
}

pub fn format_repo(registry: &str, repository: &str) -> String {
    let mut output = String::new();
    if !repository.is_empty() {
        output = repository.to_string();
    }
    if !registry.is_empty() {
        output = format!("{registry}/{output}");
    }

    output
}

pub fn pull_image(
    engine: &docker::Engine,
    image: &str,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut command = engine.subcommand("pull");
    command.arg(image);
    let out = command.run_and_get_output(msg_info)?;
    command.status_result(msg_info, out.status, Some(&out))?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImageTarget {
    pub name: String,
    pub sub: Option<String>,
}

impl ImageTarget {
    pub fn image_name(&self, repository: &str, tag: &str) -> String {
        cross::docker::image_name(&self.name, self.sub.as_deref(), repository, tag)
    }

    pub fn alt(&self) -> String {
        if let Some(sub) = &self.sub {
            format!("{}:{sub}", self.name)
        } else {
            self.name.to_string()
        }
    }

    /// Determines if this target has a ci image
    pub fn has_ci_image(&self) -> bool {
        let matrix = get_matrix();
        matrix
            .iter()
            .any(|m| m.builds_image() && m.target == self.name && m.sub == self.sub)
    }

    /// Determine if this target is a "normal" target for a triplet
    pub fn is_standard_target_image(&self) -> bool {
        let matrix = get_matrix();

        !matrix
            .iter()
            .filter(|m| m.special)
            .any(|m| m.target == self.name)
            && self.has_ci_image()
    }

    // this exists solely for zig, since we also want it as a provided target.
    /// Determine if this target has a toolchain image
    pub fn is_toolchain_image(&self) -> bool {
        !matches!(self.name.as_ref(), "cross") && self.has_ci_image()
    }

    /// Determine if this target needs to interact with the project root.
    pub fn needs_workspace_root_context(&self) -> bool {
        self.name == "cross"
    }

    pub fn is_armv6(&self) -> bool {
        matches!(
            self.name.as_str(),
            "arm-unknown-linux-gnueabi" | "arm-unknown-linux-musleabi"
        )
    }

    pub fn is_armv7(&self) -> bool {
        matches!(
            self.name.as_str(),
            "armv7-unknown-linux-gnueabihf" | "armv7-unknown-linux-musleabihf"
        )
    }
}

impl std::str::FromStr for ImageTarget {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // we designate certain targets like `x86_64-unknown-linux-gnu.centos`,
        // where `centos` is a subtype of `x86_64-unknown-linux-gnu`. however,
        // LLVM triples can also contain `.` characters, such as with
        // `thumbv8m.main-none-eabihf`, so we make sure it's only at the end.
        if let Some((target, sub)) = s.rsplit_once('.') {
            if sub.chars().all(|x| char::is_ascii_alphabetic(&x)) {
                return Ok(ImageTarget {
                    name: target.to_string(),
                    sub: Some(sub.to_string()),
                });
            }
        }

        Ok(ImageTarget {
            name: s.to_string(),
            sub: None,
        })
    }
}

impl std::fmt::Display for ImageTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(sub) = &self.sub {
            write!(f, "{}.{sub}", self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

pub fn has_nightly(msg_info: &mut MessageInfo) -> cross::Result<bool> {
    cross::cargo_command()
        .arg("+nightly")
        .run_and_get_output(msg_info)
        .map(|o| o.status.success())
}

pub fn get_channel_prefer_nightly<'a>(
    msg_info: &mut MessageInfo,
    toolchain: Option<&'a str>,
) -> cross::Result<Option<&'a str>> {
    Ok(match toolchain {
        Some(t) => Some(t),
        None => match has_nightly(msg_info)? {
            true => Some("nightly"),
            false => None,
        },
    })
}

pub fn cargo(channel: Option<&str>) -> Command {
    let mut command;
    if let Some(channel) = channel {
        command = Command::new("rustup");
        command.args(["run", channel, "cargo"]);
    } else {
        command = cross::cargo_command();
    }
    command
}

pub fn cargo_metadata(msg_info: &mut MessageInfo) -> cross::Result<cross::CargoMetadata> {
    cross::cargo_metadata_with_args(Some(Path::new(env!("CARGO_MANIFEST_DIR"))), None, msg_info)?
        .ok_or_else(|| eyre::eyre!("could not find cross workspace"))
}

pub fn project_dir(msg_info: &mut MessageInfo) -> cross::Result<PathBuf> {
    Ok(cargo_metadata(msg_info)?.workspace_root)
}

macro_rules! gha_output {
    ($fmt:literal$(, $args:expr)* $(,)?) => {
        #[cfg(not(test))]
        println!($fmt $(, $args)*);
        #[cfg(test)]
        eprintln!($fmt $(,$args)*);
    };
}

// note: for GHA actions we need to output these tags no matter the verbosity level
pub fn gha_print(content: &str) {
    gha_output!("{}", content);
}

// note: for GHA actions we need to output these tags no matter the verbosity level
pub fn gha_error(content: &str) {
    gha_output!("::error {}", content);
}

#[track_caller]
pub fn gha_output(tag: &str, content: &str) -> cross::Result<()> {
    if content.contains('\n') {
        // https://github.com/actions/toolkit/issues/403
        eyre::bail!("output `{tag}` contains newlines, consider serializing with json and deserializing in gha with fromJSON()");
    }
    write_to_gha_env_file("GITHUB_OUTPUT", &format!("{tag}={content}"))?;
    Ok(())
}

pub fn read_dockerfiles(msg_info: &mut MessageInfo) -> cross::Result<Vec<(PathBuf, String)>> {
    let root = project_dir(msg_info)?;
    let docker = root.join("docker");
    let mut dockerfiles = vec![];
    for entry in fs::read_dir(docker)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let file_name = entry.file_name();
        if file_type.is_file() && file_name.to_utf8()?.starts_with("Dockerfile") {
            let contents = fs::read_to_string(entry.path())?;
            dockerfiles.push((entry.path().to_path_buf(), contents));
        }
    }

    Ok(dockerfiles)
}

pub fn write_to_string(path: &Path, contents: &str) -> cross::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)?;
    writeln!(file, "{}", contents)?;
    Ok(())
}

// https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions#environment-files
pub fn write_to_gha_env_file(env_name: &str, contents: &str) -> cross::Result<()> {
    eprintln!("{contents}");
    let path = if let Ok(path) = env::var(env_name) {
        PathBuf::from(path)
    } else {
        eyre::ensure!(
            env::var("GITHUB_ACTIONS").is_err(),
            "expected GHA envfile to exist"
        );
        return Ok(());
    };
    let mut file = fs::OpenOptions::new().append(true).open(path)?;
    writeln!(file, "{}", contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use cross::shell::Verbosity;
    use std::collections::BTreeMap;

    #[test]
    fn test_parse_image_target() {
        assert_eq!(
            ImageTarget {
                name: "x86_64-unknown-linux-gnu".to_owned(),
                sub: None,
            },
            "x86_64-unknown-linux-gnu".parse().unwrap()
        );
        assert_eq!(
            ImageTarget {
                name: "x86_64-unknown-linux-gnu".to_owned(),
                sub: Some("centos".to_owned()),
            },
            "x86_64-unknown-linux-gnu.centos".parse().unwrap()
        );
        assert_eq!(
            ImageTarget {
                name: "thumbv8m.main-none-eabihf".to_owned(),
                sub: None,
            },
            "thumbv8m.main-none-eabihf".parse().unwrap()
        );
        assert_eq!(
            ImageTarget {
                name: "thumbv8m.main-unknown-linux-gnueabihf".to_owned(),
                sub: Some("alpine".to_owned()),
            },
            "thumbv8m.main-unknown-linux-gnueabihf.alpine"
                .parse()
                .unwrap()
        );
    }

    #[test]
    fn check_ubuntu_base() -> cross::Result<()> {
        // count all the entries of FROM for our images
        let mut counts = BTreeMap::new();
        let mut msg_info = Verbosity::Verbose(2).into();
        let dockerfiles = read_dockerfiles(&mut msg_info)?;
        for (path, dockerfile) in dockerfiles {
            let lines: Vec<&str> = dockerfile.lines().collect();
            let index = lines
                .iter()
                .map(|x| x.trim())
                .position(|x| x.to_lowercase().starts_with("from"))
                .ok_or_else(|| eyre::eyre!("unable to find FROM instruction for {:?}", path))?;
            let tag = lines[index]
                .split_whitespace()
                .nth(1)
                .ok_or_else(|| eyre::eyre!("invalid FROM instruction, got {}", lines[index]))?;
            if let Some(value) = counts.get_mut(tag) {
                *value += 1;
            } else {
                counts.insert(tag.to_string(), 1);
            }
        }

        // Now, get the most common and ensure our base is correct.
        let actual_base = cross::docker::UBUNTU_BASE;
        let max_base = counts
            .iter()
            .max_by(|x, y| x.1.cmp(y.1))
            .map(|(k, _)| k)
            .ok_or_else(|| eyre::eyre!("have no dockerfiles"))?;

        if actual_base != max_base {
            eyre::bail!("most common base image is {max_base} but source code has {actual_base}")
        } else {
            Ok(())
        }
    }
}
