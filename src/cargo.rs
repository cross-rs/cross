use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::{env, fs};

use errors::*;
use extensions::CommandExt;

/// Cargo project root
pub fn root() -> Result<Option<PathBuf>> {
    let cd = env::current_dir().chain_err(|| "couldn't get current directory")?;

    let mut dir = &*cd;
    loop {
        let toml = dir.join("Cargo.toml");

        if fs::metadata(&toml).is_ok() {
            return Ok(Some(dir.to_owned()));
        }

        match dir.parent() {
            Some(p) => dir = p,
            None => break,
        }
    }

    Ok(None)
}

/// Pass-through mode
pub fn run(args: &[String]) -> Result<ExitStatus> {
    Command::new("cargo").args(args).run_and_get_exit_status()
}
