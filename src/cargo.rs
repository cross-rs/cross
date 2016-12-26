use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::{env, fs};

use errors::*;
use extensions::CommandExt;

#[derive(Clone, Copy)]
pub enum Subcommand {
    Build,
    Other,
    Run,
    Rustc,
    Test,
}

impl Subcommand {
    pub fn needs_docker(&self) -> bool {
        match *self {
            Subcommand::Other => false,
            _ => true,
        }
    }

    pub fn needs_qemu(&self) -> bool {
        match *self {
            Subcommand::Run | Subcommand::Test => true,
            _ => false,
        }
    }
}

impl<'a> From<&'a str> for Subcommand {
    fn from(s: &str) -> Subcommand {
        match s {
            "build" => Subcommand::Build,
            "run" => Subcommand::Run,
            "rustc" => Subcommand::Rustc,
            "test" => Subcommand::Test,
            _ => Subcommand::Other,
        }
    }
}

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
