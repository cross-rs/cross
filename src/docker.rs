use std::{env, fs};
use std::path::Path;
use std::process::{Command, ExitStatus};

use errors::*;
use extensions::CommandExt;
use id;
use rustc;

pub fn run(target: &str,
           args: &[String],
           cargo_root: &Path)
           -> Result<ExitStatus> {
    let cargo_dir = env::home_dir()
        .ok_or_else(|| "couldn't get home directory. Is $HOME not set?")?
        .join(".cargo");
    let target_dir = cargo_root.join("target");

    // create the target directory if it doesn't exist, otherwise `docker` will
    // create it but it will be owned by `root`
    fs::create_dir(&target_dir).ok();

    let mut cmd = Command::new("cargo");
    cmd.args(args);

    Command::new("docker")
        .arg("run")
        .arg("--rm")
        .args(&["--user", &format!("{}:{}", id::user(), id::group())])
        .args(&["-e", "CARGO_HOME=/cargo"])
        .args(&["-e", "CARGO_TARGET_DIR=/target"])
        .args(&["-v", &format!("{}:/cargo", cargo_dir.display())])
        .args(&["-v", &format!("{}:/project:ro", cargo_root.display())])
        .args(&["-v", &format!("{}:/rust:ro", rustc::sysroot()?.display())])
        .args(&["-v", &format!("{}:/target", target_dir.display())])
        .args(&["-w", "/project"])
        .args(&["-it", &format!("japaric/{}", target)])
        .args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)])
        .run_and_get_exit_status()
}
