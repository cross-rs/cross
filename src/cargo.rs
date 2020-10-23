use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::{env, fs};

use crate::errors::*;
use crate::extensions::CommandExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subcommand {
    Build,
    Check,
    Doc,
    Other,
    Run,
    Rustc,
    Test,
    Bench,
    Deb,
    Clippy,
    Metadata,
}

impl Subcommand {
    pub fn needs_docker(self) -> bool {
        match self {
            Subcommand::Other => false,
            _ => true,
        }
    }

    pub fn needs_interpreter(self) -> bool {
        match self {
            Subcommand::Run | Subcommand::Test | Subcommand::Bench => true,
            _ => false,
        }
    }

    pub fn needs_target_in_command(self) -> bool {
        match self {
            Subcommand::Metadata => false,
            _ => true,
        }
    }
}

impl<'a> From<&'a str> for Subcommand {
    fn from(s: &str) -> Subcommand {
        match s {
            "build" => Subcommand::Build,
            "check" => Subcommand::Check,
            "doc" => Subcommand::Doc,
            "run" => Subcommand::Run,
            "rustc" => Subcommand::Rustc,
            "test" => Subcommand::Test,
            "bench" => Subcommand::Bench,
            "deb" => Subcommand::Deb,
            "clippy" => Subcommand::Clippy,
            "metadata" => Subcommand::Metadata,
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
