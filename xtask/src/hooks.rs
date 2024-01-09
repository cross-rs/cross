use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::{cargo, cargo_metadata, get_channel_prefer_nightly};
use clap::builder::BoolishValueParser;
use clap::Args;
use cross::shell::MessageInfo;
use cross::CommandExt;
use eyre::Context;

const CARGO_FLAGS: &[&str] = &["--all-features", "--all-targets", "--workspace"];

#[derive(Args, Debug)]
pub struct Check {
    /// Run shellcheck on all files, not just staged files.
    #[clap(short, long)]
    all: bool,
    /// Run Python linter checks.
    #[clap(short, long, env = "PYTHON", value_parser = BoolishValueParser::new())]
    python: bool,
    /// Flake8 command (either an executable or list of arguments)
    #[clap(short, long, env = "FLAKE8")]
    flake8: Option<String>,
}

#[derive(Args, Debug)]
pub struct Test {
    /// Run Python test suite.
    #[clap(short, long, env = "PYTHON", value_parser = BoolishValueParser::new())]
    python: bool,
    /// Tox command (either an executable or list of arguments)
    #[clap(short, long, env = "TOX")]
    tox: Option<String>,
}

#[track_caller]
fn cargo_fmt(msg_info: &mut MessageInfo, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .args(["fmt", "--", "--check"])
        .run(msg_info, false)
}

#[track_caller]
fn cargo_clippy(msg_info: &mut MessageInfo, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .arg("clippy")
        .args(CARGO_FLAGS)
        .args(["--", "--deny", "warnings"])
        .run(msg_info, false)
}

#[track_caller]
fn cargo_test(msg_info: &mut MessageInfo, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .arg("test")
        .args(CARGO_FLAGS)
        .run(msg_info, false)
}

fn splitlines(string: String) -> Vec<String> {
    string.lines().map(|l| l.to_string()).collect()
}

fn staged_files(msg_info: &mut MessageInfo) -> cross::Result<Vec<String>> {
    Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACM"])
        .run_and_get_stdout(msg_info)
        .map(splitlines)
}

fn all_files(msg_info: &mut MessageInfo) -> cross::Result<Vec<String>> {
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

fn shellcheck(all: bool, msg_info: &mut MessageInfo) -> cross::Result<()> {
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

fn parse_command(value: &str) -> cross::Result<Vec<String>> {
    shell_words::split(value).wrap_err_with(|| format!("could not parse command of {}", value))
}

fn python_dir(metadata: &cross::CargoMetadata) -> PathBuf {
    metadata.workspace_root.join("docker").join("android")
}

fn python_env(cmd: &mut Command, metadata: &cross::CargoMetadata) {
    cmd.env("PYTHONDONTWRITEBYTECODE", "1");
    cmd.env(
        "PYTHONPYCACHEPREFIX",
        metadata.target_directory.join("__pycache__"),
    );
}

fn python_lint(flake8: Option<&str>, msg_info: &mut MessageInfo) -> cross::Result<()> {
    let metadata = cargo_metadata(msg_info)?;
    let args = flake8
        .map(parse_command)
        .unwrap_or_else(|| Ok(vec!["flake8".to_owned()]))?;
    let mut cmd = Command::new(
        args.first()
            .ok_or_else(|| eyre::eyre!("empty string provided for flake8 command"))?,
    );
    cmd.args(&args[1..]);
    python_env(&mut cmd, &metadata);
    if msg_info.is_verbose() {
        cmd.arg("--verbose");
    }
    cmd.current_dir(python_dir(&metadata));
    cmd.run(msg_info, false)?;

    Ok(())
}

fn python_test(tox: Option<&str>, msg_info: &mut MessageInfo) -> cross::Result<()> {
    let metadata = cargo_metadata(msg_info)?;
    let args = tox
        .map(parse_command)
        .unwrap_or_else(|| Ok(vec!["tox".to_owned()]))?;
    let mut cmd = Command::new(
        args.first()
            .ok_or_else(|| eyre::eyre!("empty string provided for tox command"))?,
    );
    cmd.args(&args[1..]);
    cmd.args(["-e", "py3"]);
    python_env(&mut cmd, &metadata);
    cmd.arg("--workdir");
    cmd.arg(&metadata.target_directory);
    if msg_info.is_verbose() {
        cmd.arg("--verbose");
    }
    cmd.current_dir(python_dir(&metadata));
    cmd.run(msg_info, false)?;

    Ok(())
}

pub fn check(
    Check {
        all,
        python,
        flake8,
        ..
    }: Check,
    toolchain: Option<&str>,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut checks = vec!["rustfmt", "clippy", "shellcheck"];
    if python {
        checks.push("python");
    }
    msg_info.info(format_args!("Running {} checks.", checks.join(", ")))?;

    let channel = get_channel_prefer_nightly(msg_info, toolchain)?;
    cargo_fmt(msg_info, channel).wrap_err("fmt failed")?;
    cargo_clippy(msg_info, channel).wrap_err("clippy failed")?;
    shellcheck(all, msg_info).wrap_err("shellcheck failed")?;
    if python {
        python_lint(flake8.as_deref(), msg_info)?;
    }

    Ok(())
}

pub fn test(
    Test { python, tox, .. }: Test,
    toolchain: Option<&str>,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut tests = vec!["rustfmt", "unit"];
    if python {
        tests.push("python");
    }
    msg_info.info(format_args!("Running {} tests.", tests.join(", ")))?;

    let channel = get_channel_prefer_nightly(msg_info, toolchain)?;
    cargo_fmt(msg_info, channel)?;
    cargo_test(msg_info, channel)?;
    if python {
        python_test(tox.as_deref(), msg_info)?;
    }

    Ok(())
}
