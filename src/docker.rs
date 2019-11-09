use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::{env, fs};

use atty::Stream;
use error_chain::bail;

use crate::{Target, Toml};
use crate::cargo::Root;
use crate::errors::*;
use crate::extensions::CommandExt;
use crate::id;

const DOCKER_IMAGES: &[&str] = &include!(concat!(env!("OUT_DIR"), "/docker-images.rs"));
const DOCKER: &str = "docker";

pub fn docker_command(container_engine: &str, subcommand: &str) -> Command {
    let mut docker = Command::new(container_engine);
    docker.arg(subcommand);

    // We always add the `---userns host` flag for compatibility with Podman.
    docker.args(&["--userns", "host"]);
    docker
}

/// Register binfmt interpreters
pub fn register(target: &Target, toml: Option<&Toml>, verbose: bool) -> Result<()> {
    let cmd = if target.is_windows() {
        // https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html
        "mount binfmt_misc -t binfmt_misc /proc/sys/fs/binfmt_misc && \
            echo ':wine:M::MZ::/usr/bin/run-detectors:' > /proc/sys/fs/binfmt_misc/register"
    } else {
        "apt-get update && apt-get install --no-install-recommends -y \
            binfmt-support qemu-user-static"
    };

    let mut container_engine = DOCKER.to_string();
    if let Some(toml) = toml {
        if let Some(engine) = toml.container_engine(target)? {
            container_engine = engine;
        }
    }

    docker_command(&container_engine, "run")
        .arg("--privileged")
        .arg("--rm")
        .arg("ubuntu:16.04")
        .args(&["sh", "-c", cmd])
        .run(verbose)
}

pub fn run(target: &Target,
           args: &[String],
           root: &Root,
           toml: Option<&Toml>,
           uses_xargo: bool,
           sysroot: &PathBuf,
           verbose: bool)
           -> Result<ExitStatus> {
    let root = root.path();
    let home_dir = home::home_dir().ok_or_else(|| "could not find home directory")?;
    let cargo_dir = home::cargo_home()?;
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

    let mut runner = None;
    let mut container_engine = DOCKER.to_string();

    Command::new("cargo").args(&["fetch",
                "--manifest-path",
                &cargo_toml.display().to_string()])
        .run(verbose)
        .chain_err(|| "couldn't generate Cargo.lock")?;

    if let Some(toml) = toml {
        runner = toml.runner(target)?;
        if let Some(engine) = toml.container_engine(target)? {
            container_engine = engine;
        }
    }
    let mut docker = docker_command(&container_engine, "run");

    if let Some(toml) = toml {
        for var in toml.env_passthrough(target)? {
            if var.contains('=') {
                bail!("environment variable names must not contain the '=' character");
            }

            if var == "CROSS_RUNNER" {
                bail!(
                    "CROSS_RUNNER environment variable name is reserved and cannot be pass through"
                );
            }

            // Only specifying the environment variable name in the "-e"
            // flag forwards the value from the parent shell
            docker.args(&["-e", var]);
        }
    }

    docker.arg("--rm");

    // We need to specify the user for Docker, but not for Podman.
    if container_engine == DOCKER {
        docker.args(&["--user", &format!("{}:{}", id::user(), id::group())]);
    }

    docker
        .args(&["-e", "XARGO_HOME=/xargo"])
        .args(&["-e", "CARGO_HOME=/cargo"])
        .args(&["-e", "CARGO_TARGET_DIR=/target"])
        .args(&["-e", &format!("USER={}", id::username().unwrap().unwrap())]);

    if let Ok(value) = env::var("QEMU_STRACE") {
        docker.args(&["-e", &format!("QEMU_STRACE={}", value)]);
    }

    if let Ok(value) = env::var("CROSS_DEBUG") {
        docker.args(&["-e", &format!("CROSS_DEBUG={}", value)]);
    }

    if let Ok(value) = env::var("DOCKER_OPTS") {
        let opts: Vec<&str> = value.split(" ").collect();
        docker.args(&opts);
    }

    docker
        .args(&["-e", &format!("CROSS_RUNNER={}", runner.unwrap_or_else(|| String::new()))])
        .args(&["-v", &format!("{}:/xargo:Z", xargo_dir.display())])
        .args(&["-v", &format!("{}:/cargo:Z", cargo_dir.display())]);

    // Prevent `bin` from being mounted inside the Docker container. This is
    // not required for Podman.
    if container_engine == DOCKER {
        docker.args(&["-v", "/cargo/bin"]);
    }

    docker
        .args(&["-v", &format!("{}:/project:Z,ro", root.display())])
        .args(&["-v", &format!("{}:/rust:Z,ro", sysroot.display())])
        .args(&["-v", &format!("{}:/target:Z", target_dir.display())])
        .args(&["-w", "/project"]);

    if atty::is(Stream::Stdin) {
        docker.arg("-i");
        if atty::is(Stream::Stdout) && atty::is(Stream::Stderr) {
            docker.arg("-t");
        }
    }

    docker
        .arg(&image(toml, target)?)
        .args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)])
        .run_and_get_status(verbose)
}

pub fn image(toml: Option<&Toml>, target: &Target) -> Result<String> {
    if let Some(toml) = toml {
        if let Some(image) = toml.image(target)?.map(|s| s.to_owned()) {
            return Ok(image)
        }
    }

    let triple = target.triple();

    if !DOCKER_IMAGES.contains(&triple) {
        bail!("`cross` does not provide a Docker image for target {}, \
               specify a custom image in `Cross.toml`.", triple);
    }

    let version = env!("CARGO_PKG_VERSION");

    let image = if version.contains("alpha") || version.contains("dev") {
        format!("rustembedded/cross:{}", triple)
    } else {
        format!("rustembedded/cross:{}-{}", triple, version)
    };

    Ok(image)
}
