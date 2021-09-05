use std::process::Command;

use crate::Target;
use crate::errors::*;
use crate::extensions::CommandExt;

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

    Ok(out.lines().map(|l| l.replace(" (default)", "").replace(" (override)", "").trim().to_owned()).collect())
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

    Ok(AvailableTargets { default, installed, not_installed })
}

pub fn install_toolchain(toolchain: &str, verbose: bool) -> Result<()> {
    Command::new("rustup")
        .args(&["toolchain", "add", toolchain, "--profile", "minimal"])
        .run(verbose)
        .chain_err(|| format!("couldn't install toolchain `{}`", toolchain))
}

pub fn install(target: &Target, toolchain: &str, verbose: bool) -> Result<()> {
    let target = target.triple();

    Command::new("rustup")
        .args(&["target", "add", target, "--toolchain", toolchain])
        .run(verbose)
        .chain_err(|| format!("couldn't install `std` for {}", target))
}

pub fn install_component(component: &str, toolchain: &str, verbose: bool) -> Result<()> {
    Command::new("rustup")
        .args(&["component", "add", component, "--toolchain", toolchain])
        .run(verbose)
        .chain_err(|| format!("couldn't install the `{}` component", component))
}

pub fn component_is_installed(component: &str, toolchain: &str, verbose: bool) -> Result<bool> {
    Ok(Command::new("rustup")
        .args(&["component", "list", "--toolchain", toolchain])
        .run_and_get_stdout(verbose)?
        .lines()
        .any(|l| l.starts_with(component) && l.contains("installed")))
}
