use std::borrow::Cow;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::{env, fs};

use {Target, Toml};
use cargo::Root;
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

pub fn run(target: &Target,
           args: &[String],
           root: &Root,
           toml: Option<&Toml>,
           uses_xargo: bool,
           verbose: bool)
           -> Result<ExitStatus> {
    let root = root.path();
    let home_dir = env::home_dir().ok_or_else(|| "couldn't get home directory. Is $HOME not set?")?;
    let cargo_dir = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.join(".cargo"));
    let xargo_dir = env::var_os("XARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.join(".xargo"));
    let target_dir = root.join("target");

    // create the directories we are going to mount before we mount them,
    // otherwise `docker` will create them but they will be owned by `root`
    fs::create_dir(&target_dir).ok();
    fs::create_dir(&cargo_dir).ok();
    fs::create_dir(&xargo_dir).ok();

    let mut cmd = if uses_xargo {
        Command::new("xargo")
    } else {
        Command::new("cargo")
    };
    cmd.args(args);

    // We create/regenerate the lockfile on the host system because the Docker
    // container doesn't have write access to the root of the Cargo project
    let cargo_toml = root.join("Cargo.toml");
    Command::new("cargo").args(&["fetch",
                "--manifest-path",
                &cargo_toml.display().to_string()])
        .run(verbose)
        .chain_err(|| "couldn't generate Cargo.lock")?;

    let mut docker = Command::new("docker");

    docker
        .arg("run")
        .arg("--rm")
        .args(&["--user", &format!("{}:{}", id::user(), id::group())])
        .args(&["-e", "CARGO_HOME=/cargo"])
        .args(&["-e", "CARGO_TARGET_DIR=/target"])
        .args(&["-e", &format!("USER={}", id::username())]);

    if let Some(strace) = env::var("QEMU_STRACE").ok() {
        docker.args(&["-e", &format!("QEMU_STRACE={}", strace)]);
    }

    if let Some(toml) = toml {
        if toml.env_whitelist_all()?.unwrap_or(false) {
            for (var, _) in env::vars() {
                // Only specifying the environment variable name in the "-e"
                // flag forwards the value from the parent shell
                docker.args(&["-e", &var]);
            }
        } else {
            for var in toml.env_whitelist()? {
                if var.contains("=") {
                    return Err("environment variable names must not contain the '=' character".into())
                }

                // Only specifying the environment variable name in the "-e"
                // flag forwards the value from the parent shell
                docker.args(&["-e", var]);
            }
        }
    }

    docker
        .args(&["-e", "XARGO_HOME=/xargo"])
        .args(&["-v", &format!("{}:/xargo", xargo_dir.display())])
        .args(&["-v", &format!("{}:/cargo", cargo_dir.display())])
        .args(&["-v", &format!("{}:/project:ro", root.display())])
        .args(&["-v", &format!("{}:/rust:ro", rustc::sysroot(verbose)?.display())])
        .args(&["-v", &format!("{}:/target", target_dir.display())])
        .args(&["-w", "/project"])
        .args(&["-it", &image(toml, target)?])
        .args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)])
        .run_and_get_status(verbose)
}

fn image(toml: Option<&Toml>, target: &Target) -> Result<String> {
    Ok(if let Some(toml) = toml {
            toml.image(target)?.map(|s| s.to_owned())
        } else {
            None
        }
        .unwrap_or_else(|| {
            let version = env!("CARGO_PKG_VERSION");
            let tag = if version.ends_with("-dev") {
                Cow::from("latest")
            } else {
                Cow::from(format!("v{}", version))
            };
            format!("japaric/{}:{}", target.triple(), tag)
        }))
}
