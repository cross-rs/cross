use std::path::Path;
use std::process::Command;

use rustc_version::Version;

use crate::errors::*;
use crate::extensions::CommandExt;
use crate::Target;

#[derive(Debug)]
pub struct AvailableTargets {
    default: String,
    installed: Vec<String>,
    not_installed: Vec<String>,
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

pub fn installed_toolchains(verbose: bool) -> Result<Vec<String>> {
    let out = Command::new("rustup")
        .args(&["toolchain", "list"])
        .run_and_get_stdout(verbose)?;

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

pub fn available_targets(toolchain: &str, verbose: bool) -> Result<AvailableTargets> {
    let out = Command::new("rustup")
        .args(&["target", "list", "--toolchain", toolchain])
        .run_and_get_stdout(verbose)?;

    let mut default = String::new();
    let mut installed = vec![];
    let mut not_installed = vec![];

    for line in out.lines() {
        let target = line.split(' ').next().unwrap().to_string();
        if line.contains("(default)") {
            assert!(default.is_empty());
            default = target;
        } else if line.contains("(installed)") {
            installed.push(target)
        } else {
            not_installed.push(target)
        }
    }

    Ok(AvailableTargets {
        default,
        installed,
        not_installed,
    })
}

pub fn install_toolchain(toolchain: &str, verbose: bool) -> Result<()> {
    Command::new("rustup")
        .args(&["toolchain", "add", toolchain, "--profile", "minimal"])
        .run(verbose)
        .wrap_err_with(|| format!("couldn't install toolchain `{toolchain}`"))
}

pub fn install(target: &Target, toolchain: &str, verbose: bool) -> Result<()> {
    let target = target.triple();

    Command::new("rustup")
        .args(&["target", "add", target, "--toolchain", toolchain])
        .run(verbose)
        .wrap_err_with(|| format!("couldn't install `std` for {target}"))
}

pub fn install_component(component: &str, toolchain: &str, verbose: bool) -> Result<()> {
    Command::new("rustup")
        .args(&["component", "add", component, "--toolchain", toolchain])
        .run(verbose)
        .wrap_err_with(|| format!("couldn't install the `{component}` component"))
}

pub fn component_is_installed(component: &str, toolchain: &str, verbose: bool) -> Result<bool> {
    Ok(Command::new("rustup")
        .args(&["component", "list", "--toolchain", toolchain])
        .run_and_get_stdout(verbose)?
        .lines()
        .any(|l| l.starts_with(component) && l.contains("installed")))
}

pub fn rustc_version(toolchain_path: &Path) -> Result<Option<(Version, Option<String>)>> {
    let path = toolchain_path.join("lib/rustlib/multirust-channel-manifest.toml");
    if path.exists() {
        let contents = std::fs::read(&path)
            .wrap_err_with(|| format!("couldn't open file `{}`", path.display()))?;
        let manifest: toml::value::Table = toml::from_slice(&contents)?;
        if let Some(rust_version) = manifest
            .get("pkg")
            .and_then(|pkg| pkg.get("rust"))
            .and_then(|rust| rust.get("version"))
            .and_then(|version| version.as_str())
        {
            let mut i = rust_version.splitn(2, ' ');
            Ok(Some((
                Version::parse(
                    i.next().ok_or_else(|| {
                        eyre::eyre!("no rust version found in {}", path.display())
                    })?,
                )?,
                i.next().map(|s| s.to_owned()),
            )))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}
