use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use crate::cli::Args;
use crate::errors::*;
use crate::extensions::{env_program, CommandExt};

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
    Clean,
}

impl Subcommand {
    pub fn needs_docker(self, is_remote: bool) -> bool {
        match self {
            Subcommand::Other | Subcommand::List => false,
            Subcommand::Clean if !is_remote => false,
            _ => true,
        }
    }

    pub fn needs_host(self, is_remote: bool) -> bool {
        self == Subcommand::Clean && is_remote
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
            "clean" => Subcommand::Clean,
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
    pub target_directory: PathBuf,
    pub packages: Vec<Package>,
    pub workspace_members: Vec<String>,
}

impl CargoMetadata {
    fn non_workspace_members(&self) -> impl Iterator<Item = &Package> {
        self.packages
            .iter()
            .filter(|p| !self.workspace_members.iter().any(|m| m == &p.id))
    }

    pub fn path_dependencies(&self) -> impl Iterator<Item = &Path> {
        // TODO: Also filter out things that are in workspace, but not a workspace member
        self.non_workspace_members().filter_map(|p| p.crate_path())
    }

    #[cfg(feature = "dev")]
    pub fn get_package(&self, package: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.name == package)
    }
}

#[derive(Debug, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub manifest_path: PathBuf,
    pub source: Option<String>,
    pub version: String,
    pub license: Option<String>,
}

impl Package {
    /// Returns the absolute path to the packages manifest "folder"
    fn crate_path(&self) -> Option<&Path> {
        // when source is none, this package is a path dependency or a workspace member
        if self.source.is_none() {
            self.manifest_path.parent()
        } else {
            None
        }
    }
}

pub fn cargo_command() -> Command {
    let mut cmd = Command::new(env_program("CARGO", "cargo"));
    cmd.env(
        crate::IN_CROSS_CONTEXT_ENV,
        std::env::current_exe().unwrap_or_else(|_| "cross".to_owned().into()),
    );
    cmd
}

/// Cargo metadata with specific invocation
pub fn cargo_metadata_with_args(
    cd: Option<&Path>,
    args: Option<&Args>,
    verbose: bool,
) -> Result<Option<CargoMetadata>> {
    let mut command = cargo_command();
    command.arg("metadata").args(&["--format-version", "1"]);
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
    if let Some(target) = args.and_then(|a| a.target.as_ref()) {
        command.args(["--filter-platform", target.triple()]);
    }
    if let Some(features) = args.map(|a| &a.features).filter(|v| !v.is_empty()) {
        command.args([String::from("--features"), features.join(",")]);
    }
    let output = command.run_and_get_output(verbose)?;
    if !output.status.success() {
        // TODO: logging
        return Ok(None);
    }
    let manifest: Option<CargoMetadata> = serde_json::from_slice(&output.stdout)?;
    manifest
        .map(|m| -> Result<_> {
            Ok(CargoMetadata {
                target_directory: args
                    .and_then(|a| a.target_dir.clone())
                    .unwrap_or(m.target_directory),
                ..m
            })
        })
        .transpose()
}

/// Pass-through mode
pub fn run(args: &[String], verbose: bool) -> Result<ExitStatus, CommandError> {
    cargo_command()
        .args(args)
        .run_and_get_status(verbose, false)
}

/// run cargo and get the output, does not check the exit status
pub fn run_and_get_output(args: &[String], verbose: bool) -> Result<std::process::Output> {
    cargo_command().args(args).run_and_get_output(verbose)
}
