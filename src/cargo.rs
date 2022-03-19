use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use crate::cli::Args;
use crate::config::Config;
use crate::errors::*;
use crate::extensions::{CommandExt, OutputExt};

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

#[derive(Debug, Deserialize)]
pub struct CargoMetadata {
    pub workspace_root: PathBuf,
}

impl CargoMetadata {
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

/// Cargo metadata with specific invocation
pub fn cargo_metadata_with_args(
    cd: Option<&Path>,
    args: Option<&Args>,
) -> Result<Option<CargoMetadata>> {
    let mut command = std::process::Command::new(
        std::env::var("CARGO")
            .ok()
            .unwrap_or_else(|| "cargo".to_string()),
    );
    command.arg("metadata").arg("--format-version=1");
    if let Some(cd) = cd {
        command.current_dir(cd);
    }
    if let Some(config) = args {
        if let Some(ref manifest_path) = config.manifest_path {
            command.args(["--manifest-path".as_ref(), manifest_path.as_os_str()]);
        }
    } else {
        command.arg("--no-deps");
    }
    #[derive(Deserialize)]
    struct Manifest {
        workspace_root: PathBuf,
    }
    if let Some(cd) = cd {
        command.current_dir(cd);
    }
    let output = command.run_and_get_output(false)?;
    if !output.status.success() {
        let mut stderr = std::io::stderr();
        stderr.write_all(&output.stderr)?;
        stderr.flush()?;
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let manifest: Option<Manifest> = serde_json::from_slice(&output.stdout)?;
    Ok(manifest.map(|m| CargoMetadata {
        workspace_root: m.workspace_root,
    }))
}

/// Cargo metadata
pub fn cargo_metadata(cd: Option<&Path>) -> Result<Option<CargoMetadata>> {
    cargo_metadata_with_args(cd, None)
}

/// Pass-through mode
pub fn run(args: &[String], verbose: bool) -> Result<ExitStatus> {
    Command::new("cargo").args(args).run_and_get_status(verbose)
}

/// run cargo and get the output, does not check the exit status
pub fn run_and_get_output(args: &[String], verbose: bool) -> Result<std::process::Output> {
    Command::new("cargo").args(args).run_and_get_output(verbose)
}
