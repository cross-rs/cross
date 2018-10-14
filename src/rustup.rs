use std::process::Command;

use Target;
use errors::*;
use extensions::CommandExt;

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

pub fn available_targets(verbose: bool) -> Result<AvailableTargets> {
    let out = Command::new("rustup").args(&["target", "list"])
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

pub fn install(target: &Target, verbose: bool) -> Result<()> {
    let target = target.triple();

    Command::new("rustup")
        .args(&["target", "install", target])
        .run(verbose)
        .chain_err(|| format!("couldn't install `std` for {}", target))
}

pub fn install_rust_src(verbose: bool) -> Result<()> {
    Command::new("rustup")
        .args(&["component", "add", "rust-src"])
        .run(verbose)
        .chain_err(|| format!("couldn't install the `rust-src` component"))
}

pub fn rust_src_is_installed(verbose: bool) -> Result<bool> {
    Ok(Command::new("rustup")
        .args(&["component", "list"])
        .run_and_get_stdout(verbose)?
        .lines()
        .any(|l| l.starts_with("rust-src") && l.contains("installed")))
}
