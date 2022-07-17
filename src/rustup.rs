use std::path::PathBuf;
use std::process::Command;

use color_eyre::owo_colors::OwoColorize;
use color_eyre::SectionExt;
use rustc_version::{Channel, Version};

use crate::errors::*;
pub use crate::extensions::{CommandExt, OutputExt};
use crate::rustc::QualifiedToolchain;
use crate::shell::{MessageInfo, Verbosity};
use crate::Target;

#[derive(Debug)]
pub struct AvailableTargets {
    pub default: String,
    pub installed: Vec<String>,
    pub not_installed: Vec<String>,
}

impl AvailableTargets {
    pub fn contains(&self, target: &Target) -> bool {
        let triple = target.triple();
        self.is_installed(target) || self.not_installed.iter().any(|x| x == triple)
    }

    pub fn is_installed(&self, target: &Target) -> bool {
        let target = target.triple();
        target == self.default || self.installed.iter().any(|x| x == target)
    }
}

fn rustup_command(msg_info: &mut MessageInfo, no_flags: bool) -> Command {
    let mut cmd = Command::new("rustup");
    if no_flags {
        return cmd;
    }
    match msg_info.verbosity {
        Verbosity::Quiet => {
            cmd.arg("--quiet");
        }
        Verbosity::Verbose => {
            cmd.arg("--verbose");
        }
        _ => (),
    }
    cmd
}

pub fn active_toolchain(msg_info: &mut MessageInfo) -> Result<String> {
    let out = rustup_command(msg_info, true)
        .args(&["show", "active-toolchain"])
        .run_and_get_output(msg_info)?;

    Ok(out
        .stdout()?
        .split_once(' ')
        .ok_or_else(|| eyre::eyre!("rustup returned invalid data"))?
        .0
        .to_owned())
}

pub fn installed_toolchains(msg_info: &mut MessageInfo) -> Result<Vec<String>> {
    let out = rustup_command(msg_info, true)
        .args(&["toolchain", "list"])
        .run_and_get_stdout(msg_info)?;

    Ok(out
        .lines()
        .map(|l| {
            l.replace(" (default)", "")
                .replace(" (override)", "")
                .trim()
                .to_owned()
        })
        .collect())
}

pub fn available_targets(
    // this is explicitly a string and not `QualifiedToolchain`,
    // this is because we use this as a way to ensure that
    // the toolchain is an official toolchain, if this errors on
    // `is a custom toolchain`, we tell the user to set CROSS_CUSTOM_TOOLCHAIN
    // to handle the logic needed.
    toolchain: &str,
    msg_info: &mut MessageInfo,
) -> Result<AvailableTargets> {
    let mut cmd = rustup_command(msg_info, true);

    cmd.args(&["target", "list", "--toolchain", toolchain]);
    let output = cmd
        .run_and_get_output(msg_info)
        .suggestion("is rustup installed?")?;

    if !output.status.success() {
        if String::from_utf8_lossy(&output.stderr).contains("is a custom toolchain") {
            return Err(eyre::eyre!("`{toolchain}` is a custom toolchain.").with_section(|| r#"To use this toolchain with cross, you'll need to set the environment variable `CROSS_CUSTOM_TOOLCHAIN=1`
cross will not attempt to configure the toolchain further so that it can run your binary."#.header("Suggestion".bright_cyan())));
        }
        return Err(cmd
            .status_result(msg_info, output.status, Some(&output))
            .unwrap_err()
            .to_section_report());
    }
    let out = output.stdout()?;
    let mut default = String::new();
    let mut installed = vec![];
    let mut not_installed = vec![];

    for line in out.lines() {
        let target = line
            .split(' ')
            .next()
            .expect("rustup output should be consistent")
            .to_owned();
        if line.contains("(default)") {
            assert!(default.is_empty());
            default = target;
        } else if line.contains("(installed)") {
            installed.push(target);
        } else {
            not_installed.push(target);
        }
    }

    Ok(AvailableTargets {
        default,
        installed,
        not_installed,
    })
}

fn version(msg_info: &mut MessageInfo) -> Result<Version> {
    let out = rustup_command(msg_info, false)
        .arg("--version")
        .run_and_get_stdout(msg_info)?;

    match out
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
    {
        Some(version) => {
            semver::Version::parse(version).wrap_err_with(|| "failed to parse rustup version")
        }
        None => eyre::bail!("failed to get rustup version"),
    }
}

pub fn install_toolchain(toolchain: &QualifiedToolchain, msg_info: &mut MessageInfo) -> Result<()> {
    let mut command = rustup_command(msg_info, false);
    let toolchain = toolchain.to_string();
    command.args(&["toolchain", "add", &toolchain, "--profile", "minimal"]);
    if version(msg_info)? >= semver::Version::new(1, 25, 0) {
        command.arg("--force-non-host");
    }
    command
        .run(msg_info, false)
        .wrap_err_with(|| format!("couldn't install toolchain `{toolchain}`"))
}

pub fn install(
    target: &Target,
    toolchain: &QualifiedToolchain,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    let target = target.triple();
    let toolchain = toolchain.to_string();
    rustup_command(msg_info, false)
        .args(&["target", "add", target, "--toolchain", &toolchain])
        .run(msg_info, false)
        .wrap_err_with(|| format!("couldn't install `std` for {target}"))
}

pub fn install_component(
    component: &str,
    toolchain: &QualifiedToolchain,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    let toolchain = toolchain.to_string();
    rustup_command(msg_info, false)
        .args(&["component", "add", component, "--toolchain", &toolchain])
        .run(msg_info, false)
        .wrap_err_with(|| format!("couldn't install the `{component}` component"))
}

#[derive(Debug)]
pub enum Component<'a> {
    Installed(&'a str),
    Available(&'a str),
    NotAvailable(&'a str),
}

impl<'a> Component<'a> {
    pub fn is_installed(&'a self) -> bool {
        matches!(self, Component::Installed(_))
    }

    pub fn is_not_available(&'a self) -> bool {
        matches!(self, Component::NotAvailable(_))
    }
}

pub fn check_component<'a>(
    component: &'a str,
    toolchain: &QualifiedToolchain,
    msg_info: &mut MessageInfo,
) -> Result<Component<'a>> {
    Ok(Command::new("rustup")
        .args(&["component", "list", "--toolchain", &toolchain.to_string()])
        .run_and_get_stdout(msg_info)?
        .lines()
        .find_map(|line| {
            let available = line.starts_with(component);
            let installed = line.contains("installed");
            match available {
                true => Some(installed),
                false => None,
            }
        })
        .map_or_else(
            || Component::NotAvailable(component),
            |installed| match installed {
                true => Component::Installed(component),
                false => Component::Available(component),
            },
        ))
}

pub fn component_is_installed(
    component: &str,
    toolchain: &QualifiedToolchain,
    msg_info: &mut MessageInfo,
) -> Result<bool> {
    Ok(check_component(component, toolchain, msg_info)?.is_installed())
}

fn rustc_channel(version: &Version) -> Result<Channel> {
    match version
        .pre
        .split('.')
        .next()
        .expect("rust prerelease version should contain `.`")
    {
        "" => Ok(Channel::Stable),
        "dev" => Ok(Channel::Dev),
        "beta" => Ok(Channel::Beta),
        "nightly" => Ok(Channel::Nightly),
        x => eyre::bail!("unknown prerelease tag {x}"),
    }
}

impl QualifiedToolchain {
    fn multirust_channel_manifest_path(&self) -> PathBuf {
        self.get_sysroot()
            .join("lib/rustlib/multirust-channel-manifest.toml")
    }

    pub fn rustc_version_string(&self) -> Result<Option<String>> {
        let path = self.multirust_channel_manifest_path();
        if path.exists() {
            let contents =
                std::fs::read(&path).wrap_err_with(|| format!("couldn't open file `{path:?}`"))?;
            let manifest: toml::value::Table = toml::from_slice(&contents)?;
            return Ok(manifest
                .get("pkg")
                .and_then(|pkg| pkg.get("rust"))
                .and_then(|rust| rust.get("version"))
                .and_then(|version| version.as_str())
                .map(|version| version.to_owned()));
        }
        Ok(None)
    }

    pub fn rustc_version(&self) -> Result<Option<(Version, Channel, String)>> {
        let path = self.multirust_channel_manifest_path();
        if let Some(rust_version) = self.rustc_version_string()? {
            // Field is `"{version} ({commit} {date})"`
            if let Some((version, meta)) = rust_version.split_once(' ') {
                let version = Version::parse(version)
                    .wrap_err_with(|| format!("invalid rust version found in {path:?}"))?;
                let channel = rustc_channel(&version)?;
                return Ok(Some((version, channel, meta.to_owned())));
            }
        }
        Ok(None)
    }
}
