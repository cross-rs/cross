use std::{env, fs};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
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
    let home_dir = env::home_dir()
        .ok_or_else(|| "couldn't get home directory. Is $HOME not set?")?;
    let cargo_dir = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.join(".cargo"));
    let xargo_dir = env::var_os("XARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.join(".xargo"));
    let target_dir = cargo_root.join("target");

    // create the directories we are going to mount before we mount them,
    // otherwise `docker` will create them but they will be owned by `root`
    fs::create_dir(&target_dir).ok();
    fs::create_dir(&cargo_dir).ok();
    fs::create_dir(&xargo_dir).ok();

    let mut cmd = if target.uses_xargo() {
        Command::new("xargo")
    } else {
        Command::new("cargo")
    };
    cmd.args(args);

    let version = env!("CARGO_PKG_VERSION");
    let tag = if version.ends_with("-dev") {
        Cow::from("latest")
    } else {
        Cow::from(format!("v{}", version))
    };

    let cargo_lock = cargo_root.join("Cargo.lock");
    if !cargo_lock.exists() {
        let cargo_toml = cargo_root.join("Cargo.toml");
        Command::new("cargo").args(&["generate-lockfile",
                    "--manifest-path",
                    &cargo_toml.display().to_string()])
            .run(verbose)
            .chain_err(|| "couldn't generate Cargo.lock")?;
    }

    Command::new("docker")
        .arg("run")
        .arg("--rm")
        .args(&["--user", &format!("{}:{}", id::user(), id::group())])
        .args(&["-e", "CARGO_HOME=/cargo"])
        .args(&["-e", "CARGO_TARGET_DIR=/target"])
        .args(&["-e", &format!("USER={}", id::username())])
        .args(&["-e", "XARGO_HOME=/xargo"])
        .args(&["-v", &format!("{}:/xargo", xargo_dir.display())])
        .args(&["-v", &format!("{}:/cargo", cargo_dir.display())])
        .args(&["-v", &format!("{}:/project:ro", cargo_root.display())])
        .args(&["-v",
                &format!("{}:/rust:ro", rustc::sysroot(verbose)?.display())])
        .args(&["-v", &format!("{}:/target", target_dir.display())])
        .args(&["-w", "/project"])
        .args(&["-it", &format!("japaric/{}:{}", target.triple(), tag)])
        .args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)])
        .run_and_get_status(verbose)
}
