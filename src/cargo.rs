use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::{env, fs};

use errors::*;
use extensions::CommandExt;

#[derive(Clone, Copy)]
pub enum Subcommand {
    Build,
    Check,
    Other,
    Run,
    Rustc,
    Test,
    Deb,
}

impl<'a> From<&'a str> for Subcommand {
    fn from(s: &str) -> Subcommand {
        match s {
            "build" => Subcommand::Build,
            "check" => Subcommand::Check,
            "run" => Subcommand::Run,
            "rustc" => Subcommand::Rustc,
            "test" => Subcommand::Test,
            "deb" => Subcommand::Deb,
            _ => Subcommand::Other,
        }
    }
}

pub struct Root {
    path: PathBuf,
}

impl Root {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Cargo project root
pub fn root() -> Result<Option<Root>> {
    let cd = env::current_dir().chain_err(|| "couldn't get current directory")?;

    let mut dir = &*cd;
    loop {
        let toml = dir.join("Cargo.toml");

        if fs::metadata(&toml).is_ok() {
            return Ok(Some(Root { path: dir.to_owned() }));
        }

        match dir.parent() {
            Some(p) => dir = p,
            None => break,
        }
    }

    Ok(None)
}

/// Pass-through mode
pub fn run(args: &[String], verbose: bool) -> Result<ExitStatus> {
    Command::new("cargo").args(args).run_and_get_status(verbose)
}
