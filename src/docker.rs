use std::{env, fs};
use std::borrow::Cow;
use std::path::Path;
use std::process::{Command, ExitStatus};

use Target;
use errors::*;
use extensions::CommandExt;
use id;
use rustc;

/// Register QEMU interpreters
pub fn register(verbose: bool) -> Result<()> {
    Command::new("docker")
        .arg("run")
        .arg("--privileged")
        .arg("--rm")
        .arg("-it")
        .arg("ubuntu:16.04")
        .args(&["sh",
                "-c",
                "apt-get update && apt-get install --no-install-recommends \
                 -y binfmt-support qemu-user-static"])
        .run(verbose)
}

pub fn run(target: Target,
           args: &[String],
           cargo_root: &Path,
           verbose: bool)
           -> Result<ExitStatus> {
    let target = target.triple();
    let cargo_dir = env::home_dir()
        .ok_or_else(|| "couldn't get home directory. Is $HOME not set?")?
        .join(".cargo");
    let target_dir = cargo_root.join("target");

    // create the target directory if it doesn't exist, otherwise `docker` will
    // create it but it will be owned by `root`
    fs::create_dir(&target_dir).ok();

    let mut cmd = Command::new("cargo");
    cmd.args(args);

    let version = env!("CARGO_PKG_VERSION");
    let tag = if version.ends_with("-dev") {
        Cow::from("latest")
    } else {
        Cow::from(format!("v{}", version))
    };

    Command::new("docker")
        .arg("run")
        .arg("--rm")
        .args(&["--user", &format!("{}:{}", id::user(), id::group())])
        .args(&["-e", "CARGO_HOME=/cargo"])
        .args(&["-e", "CARGO_TARGET_DIR=/target"])
        .args(&["-v", &format!("{}:/cargo", cargo_dir.display())])
        .args(&["-v", &format!("{}:/project:ro", cargo_root.display())])
        .args(&["-v",
                &format!("{}:/rust:ro", rustc::sysroot(verbose)?.display())])
        .args(&["-v", &format!("{}:/target", target_dir.display())])
        .args(&["-w", "/project"])
        .args(&["-it", &format!("japaric/{}:{}", target, tag)])
        .args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)])
        .run_and_get_status(verbose)
}
