#![allow(clippy::needless_borrow)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::{fs, mem};

use cross::shell::MessageInfo;
use cross::{docker, CommandExt, ToUtf8};
use once_cell::sync::Lazy;
use regex::Regex;
use snapbox::assert_matches;
use snapbox::cmd::cargo_bin;

pub static ANSI_COLOR_CODES: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*m").unwrap());

macro_rules! stdout_contains {
    ($stdout:expr, $value:expr) => {{
        let stdout = $stdout;
        let replaced = ANSI_COLOR_CODES.replace_all(&stdout, "");
        assert!(replaced.contains($value), "unexpected stdout of {stdout:?}");
    }};
}

macro_rules! stderr_contains {
    ($stderr:expr, $value:expr) => {{
        let stderr = $stderr;
        let replaced = ANSI_COLOR_CODES.replace_all(&stderr, "");
        assert!(replaced.contains($value), "unexpected stderr of {stderr:?}");
    }};
}

#[test]
#[ignore]
fn cli_tests() -> cross::Result<()> {
    let target = default_target();

    let cargo_version = run_success("cargo", &["--version"])?;
    let cross_version = cross::version();

    // FIXME: `[CARGOHELP]` doesn't currently work.
    // https://github.com/assert-rs/trycmd/issues/170
    trycmd::TestCases::new()
        .case("tests/cmd/*.toml")
        .case("tests/cmd/*.md")
        .env("CARGO_BUILD_TARGET", target)
        .insert_var("[CARGOVERSION]", cargo_version.stdout)?
        .insert_var("[CROSSVERSION]", cross_version)?;

    Ok(())
}

#[test]
#[ignore]
fn cargo_help_tests() -> cross::Result<()> {
    let cargo_help = run_success("cargo", &["--help"])?;
    let stderr = fallback()?;

    let short_help = run_success("cross", &["-h"])?;
    assert_matches(&cargo_help.stdout, &short_help.stdout);
    assert_matches(&stderr, &short_help.stderr);

    let long_help = run_success("cross", &["--help"])?;
    assert_matches(&cargo_help.stdout, &long_help.stdout);
    assert_matches(&stderr, &long_help.stderr);

    let list_help = run_success("cross", &["--list", "--help"])?;
    assert_matches(&cargo_help.stdout, &list_help.stdout);
    assert_matches(&stderr, &list_help.stderr);

    Ok(())
}

#[test]
#[ignore]
fn cargo_subcommand_help_tests() -> cross::Result<()> {
    let flags = &["search", "--help"];
    let cargo_help = run_success("cargo", flags)?;
    let stderr = fallback()?;

    let cross_help = run_success("cross", flags)?;
    assert_matches(&cargo_help.stdout, &cross_help.stdout);
    assert_matches(&stderr, &cross_help.stderr);

    Ok(())
}

#[test]
#[ignore]
fn cargo_verbose_version_tests() -> cross::Result<()> {
    let flags = &["--verbose", "--version"];
    let cargo_version = run_success("cargo", flags)?;
    let stderr = fallback()?;

    let cross_version = run_success("cross", flags)?;
    let expected = format!(
        "{}\n+ cargo --verbose --version\n{}",
        cross::version(),
        cargo_version.stdout,
    );
    assert_matches(&expected, &cross_version.stdout);
    assert_matches(&stderr, &cross_version.stderr);

    Ok(())
}

#[test]
#[ignore]
fn cross_command_test() -> cross::Result<()> {
    let target = default_target();
    let flags = &["build", "--target", &target];

    pull_default()?;
    let test_url = "https://github.com/cross-rs/test-workspace";
    let tmpdir = temp_dir(None)?;
    clone(test_url, &tmpdir)?;

    let mut msg_info = MessageInfo::default();
    msg_info.note(format_args!(
        "running in-container integration tests: this may take a while"
    ))?;

    let manifest_path = "--manifest-path=./workspace/Cargo.toml";
    let workspace_dir = format!("{tmpdir}/workspace");
    let debug_dir = format!("target/{target}/debug");

    // check custom manifest path build
    let libworkspace = format!("{debug_dir}/libtest_workspace.rlib");
    let build = Utf8Output::new(
        run_cmd("cross", flags)
            .arg(manifest_path)
            .current_dir(&tmpdir)
            .output()?,
    )?;
    assert_build_debug(&build);
    check_file_exists(&workspace_dir, &libworkspace)?;

    let build = Utf8Output::new(
        run_cmd("cross", flags)
            .arg(manifest_path)
            .arg("--quiet")
            .current_dir(&tmpdir)
            .output()?,
    )?;
    assert_build_debug_quiet(&build);
    check_file_exists(&workspace_dir, &libworkspace)?;

    let build = Utf8Output::new(
        run_cmd("cross", flags)
            .arg(manifest_path)
            .arg("--verbose")
            .current_dir(&tmpdir)
            .output()?,
    )?;
    assert_build_debug_verbose(&build);
    check_file_exists(&workspace_dir, &libworkspace)?;

    // check within a workspace
    let build = Utf8Output::new(
        run_cmd("cross", flags)
            .current_dir(&workspace_dir)
            .output()?,
    )?;
    assert_build_debug(&build);
    check_file_exists(&workspace_dir, &libworkspace)?;

    let build = Utf8Output::new(
        run_cmd("cross", flags)
            .args(["--features", "dependencies"])
            .current_dir(&workspace_dir)
            .output()?,
    )?;
    assert_build_debug(&build);
    check_file_exists(&workspace_dir, &format!("{debug_dir}/dependencies"))?;

    // check using a custom target directory
    let build = Utf8Output::new(
        run_cmd("cross", flags)
            .args(["--features", "dependencies"])
            .args(["--target-dir", "custom"])
            .current_dir(&workspace_dir)
            .output()?,
    )?;
    assert_build_debug(&build);
    let custom_dir = format!("custom/{target}/debug");
    check_file_exists(&workspace_dir, &format!("{custom_dir}/dependencies"))?;

    let flags = &["run", "--target", &target];
    let binary_dir = format!("{workspace_dir}/binary");
    let run = Utf8Output::new(run_cmd("cross", flags).current_dir(&binary_dir).output()?)?;
    stdout_contains!(run.stdout, "Hello from binary, binary/src/main.rs");
    stderr_contains!(run.stderr, BUILD_DEBUG);
    check_file_exists(&workspace_dir, &format!("{debug_dir}/binary"))?;

    Ok(())
}

fn cargo_metadata(msg_info: &mut MessageInfo) -> cross::Result<cross::CargoMetadata> {
    cross::cargo_metadata_with_args(Some(Path::new(env!("CARGO_MANIFEST_DIR"))), None, msg_info)?
        .ok_or_else(|| eyre::eyre!("could not find cross workspace"))
}

fn project_dir() -> cross::Result<PathBuf> {
    let mut msg_info = MessageInfo::default();
    Ok(cargo_metadata(&mut msg_info)?.workspace_root)
}

fn fallback() -> cross::Result<String> {
    let path = project_dir()?
        .join("tests")
        .join("cmd")
        .join("fallback.stderr");
    fs::read_to_string(path).map_err(Into::into)
}

fn default_target() -> &'static str {
    "x86_64-unknown-linux-gnu"
}

fn default_image() -> String {
    image(default_target(), "main")
}

fn image(target: &str, tag: &str) -> String {
    format!("{}/{target}:{tag}", docker::CROSS_IMAGE)
}

fn temp_dir(parent: Option<&Path>) -> cross::Result<String> {
    let parent = match parent {
        Some(parent) => parent.to_owned(),
        None => project_dir()?.join("target").join("tmp"),
    };
    fs::create_dir_all(&parent)?;
    let dir = tempfile::TempDir::new_in(&parent)?;
    let path = dir.path().to_owned();
    mem::drop(dir);

    fs::create_dir(&path)?;
    Ok(path.to_utf8()?.to_owned())
}

fn check_file_exists(base: impl AsRef<Path>, relpath: &str) -> cross::Result<()> {
    let path = base.as_ref().join(relpath);
    match path.exists() {
        true => Ok(()),
        false => eyre::bail!("path \"{relpath}\" unexpectedly does not exist"),
    }
}

fn clone(url: &str, path: &str) -> cross::Result<()> {
    let mut msg_info = MessageInfo::default();
    msg_info.note(format_args!(
        "cloning repository \"{url}\": this may take a while"
    ))?;

    let mut cmd = Command::new("git");
    cmd.args(["clone", "--depth", "1", "--recursive", url, path]);
    if !msg_info.is_verbose() {
        cmd.stderr(Stdio::null());
    }
    let verbose = msg_info.is_verbose();
    cmd.run(&mut msg_info, !verbose).map_err(Into::into)
}

fn pull(image: &str) -> cross::Result<()> {
    let mut msg_info = MessageInfo::default();
    msg_info.note(format_args!(
        "pulling container image \"{image}\": this may take a while"
    ))?;

    let engine = docker::Engine::new(None, None, &mut msg_info)?;
    let mut cmd = engine.subcommand("pull");
    cmd.arg(image);
    if !msg_info.is_verbose() {
        cmd.stderr(Stdio::null());
    }
    let verbose = msg_info.is_verbose();
    cmd.run(&mut msg_info, !verbose).map_err(Into::into)
}

fn pull_default() -> cross::Result<()> {
    pull(&default_image())
}

fn run(bin: &str, args: &[&str]) -> cross::Result<Utf8Output> {
    Utf8Output::new(run_cmd(bin, args).output()?)
}

fn run_cmd(bin: &str, args: &[&str]) -> Command {
    let mut cmd = match bin {
        "cross" => Command::new(cargo_bin("cross")),
        bin => Command::new(bin),
    };
    cmd.args(args);
    cmd
}

fn run_success(bin: &str, args: &[&str]) -> cross::Result<Utf8Output> {
    run_with_status(bin, args, Some(0))
}

fn run_with_status(bin: &str, args: &[&str], status: Option<i32>) -> cross::Result<Utf8Output> {
    let output = run(bin, args)?;
    if output.status.code() != status {
        eyre::bail!("Unexpected exit status of {:?}", output.status);
    }
    Ok(output)
}

const BUILD_DEBUG: &str = "Finished dev [unoptimized + debuginfo] target(s) in";

fn assert_build_debug(output: &Utf8Output) {
    let stderr = &output.stderr;
    assert_eq!("", &output.stdout);
    stderr_contains!(stderr, BUILD_DEBUG);
}

fn assert_build_debug_verbose(output: &Utf8Output) {
    let stdout = &output.stdout;
    let stderr = &output.stderr;
    stdout_contains!(stdout, "+ cargo metadata --format-version 1");
    stdout_contains!(stdout, "+ rustc --print sysroot");
    stdout_contains!(stdout, "+ rustup toolchain list");
    stdout_contains!(stdout, "+ rustup target list --toolchain");
    stdout_contains!(stdout, "+ rustup component list --toolchain");
    stdout_contains!(stdout, "CARGO_HOME");
    stdout_contains!(stdout, "CARGO_TARGET_DIR");
    stdout_contains!(stdout, "CROSS_RUSTC_MAJOR_VERSION");
    stdout_contains!(stdout, "CROSS_RUSTC_MINOR_VERSION");
    stdout_contains!(stdout, "CROSS_RUSTC_PATCH_VERSION");
    stdout_contains!(stdout, "CROSS_RUST_SYSROOT");
    stderr_contains!(stderr, BUILD_DEBUG);
}

fn assert_build_debug_quiet(output: &Utf8Output) {
    assert_eq!("", &output.stdout);
    assert_eq!("", &output.stderr);
}

#[allow(dead_code)]
struct Utf8Output {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl Utf8Output {
    fn new(output: Output) -> cross::Result<Self> {
        Ok(Self {
            status: output.status,
            stdout: String::from_utf8(output.stdout)?,
            stderr: String::from_utf8(output.stderr)?,
        })
    }
}
