use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;
use std::process::Command;

use crate::util::{cargo, get_channel_prefer_nightly};
use clap::Args;
use cross::shell::{self, MessageInfo};
use cross::CommandExt;

const CARGO_FLAGS: &[&str] = &["--all-features", "--all-targets", "--workspace"];

#[derive(Args, Debug)]
pub struct Check {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Run shellcheck on all files, not just staged files.
    #[clap(short, long)]
    all: bool,
}

#[derive(Args, Debug)]
pub struct Test {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
}

fn cargo_fmt(msg_info: MessageInfo, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .args(&["fmt", "--", "--check"])
        .run(msg_info, false)
        .map_err(Into::into)
}

fn cargo_clippy(msg_info: MessageInfo, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .arg("clippy")
        .args(CARGO_FLAGS)
        .args(&["--", "--deny", "warnings"])
        .run(msg_info, false)
        .map_err(Into::into)
}

fn cargo_test(msg_info: MessageInfo, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .arg("test")
        .args(CARGO_FLAGS)
        .run(msg_info, false)
        .map_err(Into::into)
}

fn splitlines(string: String) -> Vec<String> {
    string.lines().map(|l| l.to_string()).collect()
}

fn staged_files(msg_info: MessageInfo) -> cross::Result<Vec<String>> {
    Command::new("git")
        .args(&["diff", "--cached", "--name-only", "--diff-filter=ACM"])
        .run_and_get_stdout(msg_info)
        .map(splitlines)
}

fn all_files(msg_info: MessageInfo) -> cross::Result<Vec<String>> {
    Command::new("git")
        .arg("ls-files")
        .run_and_get_stdout(msg_info)
        .map(splitlines)
}

fn is_shell_script(path: impl AsRef<Path>) -> cross::Result<bool> {
    if path.as_ref().is_dir() {
        // is a directory if a git submodule
        return Ok(false);
    }
    let file = File::open(path.as_ref())?;
    let reader = BufReader::new(file);

    match reader.lines().next() {
        Some(Ok(line)) => Ok(line.starts_with("#!") && line.trim().ends_with("sh")),
        Some(Err(e)) => match e.kind() {
            // not a UTF-8 file: can't be a shell script
            ErrorKind::InvalidData => Ok(false),
            _ => Err(e.into()),
        },
        None => Ok(false),
    }
}

fn shellcheck(all: bool, msg_info: MessageInfo) -> cross::Result<()> {
    if which::which("shellcheck").is_ok() {
        let files = match all {
            true => all_files(msg_info),
            false => staged_files(msg_info),
        }?;
        let mut scripts = vec![];
        for file in files {
            if is_shell_script(&file)? {
                scripts.push(file);
            }
        }
        if !scripts.is_empty() {
            Command::new("shellcheck")
                .args(&scripts)
                .run(msg_info, false)?;
        }
    }

    Ok(())
}

pub fn check(
    Check {
        verbose,
        quiet,
        color,
        all,
    }: Check,
    toolchain: Option<&str>,
) -> cross::Result<()> {
    let msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    shell::info("Running rustfmt, clippy, and shellcheck checks.", msg_info)?;

    let channel = get_channel_prefer_nightly(msg_info, toolchain)?;
    cargo_fmt(msg_info, channel)?;
    cargo_clippy(msg_info, channel)?;
    shellcheck(all, msg_info)?;

    Ok(())
}

pub fn test(
    Test {
        verbose,
        quiet,
        color,
    }: Test,
    toolchain: Option<&str>,
) -> cross::Result<()> {
    let msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    shell::info("Running cargo fmt and tests", msg_info)?;

    let channel = get_channel_prefer_nightly(msg_info, toolchain)?;
    cargo_fmt(msg_info, channel)?;
    cargo_test(msg_info, channel)?;

    Ok(())
}
