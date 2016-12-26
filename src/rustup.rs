use std::process::Command;

use Target;
use errors::*;
use extensions::CommandExt;

pub fn install(target: Target) -> Result<()> {
    let target = target.triple();

    Command::new("rustup")
        .args(&["target", "install", target])
        .run()
        .chain_err(|| format!("couldn't install `std` for {}", target))
}

pub fn installed_targets() -> Result<Vec<Target>> {
    let out = Command::new("rustup").args(&["target", "list"])
        .run_and_get_stdout()?;

    Ok(out.lines()
        .filter_map(|line| if line.contains("installed") ||
                              line.contains("default") {
            line.splitn(2, ' ').next().map(Target::from)
        } else {
            None
        })
        .collect())
}
