use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

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
    Clippy,
    Metadata,
    List,
}

impl Subcommand {
    pub fn needs_docker(self) -> bool {
        !matches!(self, Subcommand::Other | Subcommand::List)
    }

    pub fn needs_interpreter(self) -> bool {
        matches!(self, Subcommand::Run | Subcommand::Test | Subcommand::Bench)
    }

    pub fn needs_target_in_command(self) -> bool {
        !matches!(self, Subcommand::Metadata)
    }
}

impl<'a> From<&'a str> for Subcommand {
    fn from(s: &str) -> Subcommand {
        match s {
            "b" | "build" => Subcommand::Build,
            "c" | "check" => Subcommand::Check,
            "doc" => Subcommand::Doc,
            "r" | "run" => Subcommand::Run,
            "rustc" => Subcommand::Rustc,
            "t" | "test" => Subcommand::Test,
            "bench" => Subcommand::Bench,
            "clippy" => Subcommand::Clippy,
            "metadata" => Subcommand::Metadata,
            "--list" => Subcommand::List,
            _ => Subcommand::Other,
        }
    }
}

#[derive(Debug)]
pub struct Root {
    pub path: PathBuf,
}

impl Root {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Cargo project root
pub fn root(cd: Option<&Path>) -> Result<Option<Root>> {
    #[derive(Deserialize)]
    struct Manifest {
        workspace_root: PathBuf,
    }
    let mut command = std::process::Command::new(
        std::env::var("CARGO")
            .ok()
            .unwrap_or_else(|| "cargo".to_string()),
    );
    command
        .arg("metadata")
        .arg("--format-version=1")
        .arg("--no-deps");
    if let Some(cd) = cd {
        command.current_dir(cd);
    }
    let output = command.output()?;
    let manifest: Option<Manifest> = serde_json::from_slice(&output.stdout)?;
    Ok(manifest.map(|m| Root {
        path: m.workspace_root,
    }))
}

/// Pass-through mode
pub fn run(args: &[String], verbose: bool) -> Result<ExitStatus> {
    Command::new("cargo").args(args).run_and_get_status(verbose)
}

/// run cargo and get the output, does not check the exit status
pub fn run_and_get_output(args: &[String], verbose: bool) -> Result<std::process::Output> {
    Command::new("cargo").args(args).run_and_get_output(verbose)
}
