use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;
use std::process::Command;

use clap::Args;
use cross::CommandExt;

const CARGO_FLAGS: &[&str] = &["--all-features", "--all-targets", "--workspace"];

#[derive(Args, Debug)]
pub struct Check {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    verbose: bool,
    /// Run shellcheck on all files, not just staged files.
    #[clap(short, long)]
    all: bool,
}

#[derive(Args, Debug)]
pub struct Test {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    verbose: bool,
}

fn has_nightly(verbose: bool) -> cross::Result<bool> {
    cross::cargo_command()
        .arg("+nightly")
        .run_and_get_output(verbose)
        .map(|o| o.status.success())
        .map_err(Into::into)
}

fn get_channel_prefer_nightly(
    verbose: bool,
    toolchain: Option<&str>,
) -> cross::Result<Option<&str>> {
    Ok(match toolchain {
        Some(t) => Some(t),
        None => match has_nightly(verbose)? {
            true => Some("nightly"),
            false => None,
        },
    })
}

fn cargo(channel: Option<&str>) -> Command {
    let mut command = cross::cargo_command();
    if let Some(channel) = channel {
        command.arg(&format!("+{channel}"));
    }
    command
}

fn cargo_fmt(verbose: bool, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .args(&["fmt", "--", "--check"])
        .run(verbose, false)
        .map_err(Into::into)
}

fn cargo_clippy(verbose: bool, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .arg("clippy")
        .args(CARGO_FLAGS)
        .args(&["--", "--deny", "warnings"])
        .run(verbose, false)
        .map_err(Into::into)
}

fn cargo_test(verbose: bool, channel: Option<&str>) -> cross::Result<()> {
    cargo(channel)
        .arg("test")
        .args(CARGO_FLAGS)
        .run(verbose, false)
        .map_err(Into::into)
}

fn splitlines(string: String) -> Vec<String> {
    string.lines().map(|l| l.to_string()).collect()
}

fn staged_files(verbose: bool) -> cross::Result<Vec<String>> {
    Command::new("git")
        .args(&["diff", "--cached", "--name-only", "--diff-filter=ACM"])
        .run_and_get_stdout(verbose)
        .map(splitlines)
}

fn all_files(verbose: bool) -> cross::Result<Vec<String>> {
    Command::new("git")
        .arg("ls-files")
        .run_and_get_stdout(verbose)
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

fn shellcheck(all: bool, verbose: bool) -> cross::Result<()> {
    if which::which("shellcheck").is_ok() {
        let files = match all {
            true => all_files(verbose),
            false => staged_files(verbose),
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
                .run(verbose, false)?;
        }
    }

    Ok(())
}

pub fn check(Check { verbose, all }: Check, toolchain: Option<&str>) -> cross::Result<()> {
    println!("Running rustfmt, clippy, and shellcheck checks.");

    let channel = get_channel_prefer_nightly(verbose, toolchain)?;
    cargo_fmt(verbose, channel)?;
    cargo_clippy(verbose, channel)?;
    shellcheck(all, verbose)?;

    Ok(())
}

pub fn test(Test { verbose }: Test, toolchain: Option<&str>) -> cross::Result<()> {
    println!("Running cargo fmt and tests");

    let channel = get_channel_prefer_nightly(verbose, toolchain)?;
    cargo_fmt(verbose, channel)?;
    cargo_test(verbose, channel)?;

    Ok(())
}
