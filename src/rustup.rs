use std::process::Command;

use Target;
use errors::*;
use extensions::CommandExt;

pub fn install(target: Target, verbose: bool) -> Result<()> {
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

pub fn installed_targets(verbose: bool) -> Result<Vec<Target>> {
    let out = Command::new("rustup").args(&["target", "list"])
        .run_and_get_stdout(verbose)?;

    Ok(out.lines()
        .filter_map(|line| if line.contains("installed") ||
                              line.contains("default") {
            line.splitn(2, ' ').next().map(Target::from)
        } else {
            None
        })
        .collect())
}

pub fn rust_src_is_installed(verbose: bool) -> Result<bool> {
    Ok(Command::new("rustup")
        .args(&["component", "list"])
        .run_and_get_stdout(verbose)?
        .lines()
        .any(|l| l.starts_with("rust-src") && l.contains("installed")))
}
