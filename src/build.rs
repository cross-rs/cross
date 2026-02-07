use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

struct Some {}

impl<E> From<E> for Some
where
    E: Error,
{
    fn from(_: E) -> Some {
        Some {}
    }
}

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    File::create(out_dir.join("commit-info.txt"))
        .unwrap()
        .write_all(commit_info().as_bytes())
        .unwrap();

    // Add "cross_sandboxed" to list of approved cfgs
    println!("cargo::rustc-check-cfg=cfg(cross_sandboxed)");

    if env::var("CROSS_SANDBOXED").is_ok() {
        println!("cargo:rustc-cfg=cross_sandboxed");
    }
    println!("cargo:rerun-if-env-changed=CROSS_SANDBOXED");
}

fn commit_info() -> String {
    match (commit_hash(), commit_date()) {
        (Ok(hash), Ok(date)) => format!(" ({} {})", hash.trim(), date.trim()),
        _ => String::new(),
    }
}

fn commit_hash() -> Result<String, Some> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(Some {})
    }
}

fn commit_date() -> Result<String, Some> {
    let output = Command::new("git")
        .args(["log", "-1", "--date=short", "--pretty=format:%cd"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(Some {})
    }
}
