use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::{env, fs};

use errors::*;
use extensions::CommandExt;

#[derive(Debug, Clone, Copy)]
pub enum Subcommand {
    Build,
    Check,
    Other,
    Run,
    Rustc,
    Test,
    Bench,
    Deb,
}

impl Subcommand {
    pub fn needs_docker(&self) -> bool {
        match *self {
            Subcommand::Other => false,
            _ => true,
        }
    }

    pub fn needs_interpreter(&self) -> bool {
        match *self {
            Subcommand::Run | Subcommand::Test | Subcommand::Bench => true,
            _ => false,
        }
    }
}

impl<'a> From<&'a str> for Subcommand {
    fn from(s: &str) -> Subcommand {
        match s {
            "build" => Subcommand::Build,
            "check" => Subcommand::Check,
            "run" => Subcommand::Run,
            "rustc" => Subcommand::Rustc,
            "test" => Subcommand::Test,
            "bench" => Subcommand::Bench,
            "deb" => Subcommand::Deb,
            _ => Subcommand::Other,
        }
    }
}

#[derive(Debug)]
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
