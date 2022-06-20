use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
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

    File::create(out_dir.join("docker-images.rs"))
        .unwrap()
        .write_all(docker_images().as_bytes())
        .unwrap();
}

fn commit_info() -> String {
    match (commit_hash(), commit_date()) {
        (Ok(hash), Ok(date)) => format!(" ({} {})", hash.trim(), date.trim()),
        _ => String::new(),
    }
}

fn commit_hash() -> Result<String, Some> {
    let output = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(Some {})
    }
}

fn commit_date() -> Result<String, Some> {
    let output = Command::new("git")
        .args(&["log", "-1", "--date=short", "--pretty=format:%cd"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(Some {})
    }
}

fn docker_images() -> String {
    let mut images = String::from("[");
    let mut dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    dir.push("docker");

    let dir = dir.read_dir().unwrap();
    let mut paths = dir.collect::<io::Result<Vec<_>>>().unwrap();
    paths.sort_by_key(|e| e.path());

    for entry in paths {
        let path = entry.path();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name.starts_with("Dockerfile.") {
            images.push('"');
            images.push_str(&file_name.replacen("Dockerfile.", "", 1));
            images.push_str("\", ");
        }
    }

    images.push(']');
    images
}
