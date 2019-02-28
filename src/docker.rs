use std::borrow::Cow;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::{env, fs};

use semver::{Version, VersionReq};

use {Target, Toml};
use cargo::Root;
use errors::*;
use extensions::CommandExt;
use id;

const DOCKER_IMAGES: &[&str] = &include!(concat!(env!("OUT_DIR"), "/docker-images.rs"));

lazy_static! {
    /// Retrieve the Docker Daemon version.
    ///
    /// # Panics
    /// Panics if the version cannot be retrieved or parsed
    static ref DOCKER_VERSION: Version = {
        let version_string = Command::new("docker")
                                .arg("version")
                                .arg("--format={{.Server.APIVersion}}")
                                .run_and_get_stdout(false)
                                .expect("Unable to obtain Docker version");
        // API versions don't have "patch" version
        Version::parse(&format!("{}.0", version_string.trim()))
            .expect("Cannot parse Docker engine version")
    };

    /// Version requirements for user namespace.
    ///
    /// # Panics
    /// Panics if the parsing fails
    static ref USERNS_REQUIREMENT: VersionReq = {
        VersionReq::parse(">= 1.24")
            .expect("Unable to parse version requirements")
    };
}

/// Add the `userns` flag, if needed
pub fn docker_command(subcommand: &str) -> Command {
    let mut docker = Command::new("docker");
    docker.arg(subcommand);
    if USERNS_REQUIREMENT.matches(&DOCKER_VERSION) {
        docker.args(&["--userns", "host"]);
    }
    docker
}

/// Register binfmt interpreters
pub fn register(target: &Target, verbose: bool) -> Result<()> {
    let cmd = if target.is_windows() {
        // https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html
        "mount binfmt_misc -t binfmt_misc /proc/sys/fs/binfmt_misc && \
            echo ':wine:M::MZ::/usr/bin/run-detectors:' > /proc/sys/fs/binfmt_misc/register"
    } else {
        "apt-get update && apt-get install --no-install-recommends -y \
            binfmt-support qemu-user-static"
    };
    docker_command("run")
        .arg("--privileged")
        .arg("--rm")
        .arg("-it")
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

    let mut docker = docker_command("run");

    docker
        .arg("--rm")
        .args(&["--user", &format!("{}:{}", id::user(), id::group())])
        .args(&["-e", "XARGO_HOME=/xargo"])
        .args(&["-e", "CARGO_HOME=/cargo"])
        .args(&["-e", "CARGO_TARGET_DIR=/target"])
        .args(&["-e", &format!("USER={}", id::username())]);

    if let Some(value) = env::var("QEMU_STRACE").ok() {
        docker.args(&["-e", &format!("QEMU_STRACE={}", value)]);
    }

    if let Some(value) = env::var("CROSS_DEBUG").ok() {
        docker.args(&["-e", &format!("CROSS_DEBUG={}", value)]);
    }

    if let Some(value) = env::var("DOCKER_OPTS").ok() {
        let opts: Vec<&str> = value.split(" ").collect();
        docker.args(&opts);
    }

    let mut runner = None;

    if let Some(toml) = toml {
        for var in toml.env_passthrough(target)? {
            if var.contains("=") {
                bail!("environment variable names must not contain the '=' character");
            }

            if var == "CROSS_RUNNER" {
                bail!("CROSS_RUNNER environment variable name is reserved and cannot be pass through");
            }

            // Only specifying the environment variable name in the "-e"
            // flag forwards the value from the parent shell
            docker.args(&["-e", var]);
        }

        runner = toml.runner(target)?;
    }

    docker
        .args(&["-e", &format!("CROSS_RUNNER={}", runner.unwrap_or_else(|| String::new()))])
        .args(&["-v", &format!("{}:/xargo", xargo_dir.display())])
        .args(&["-v", &format!("{}:/cargo", cargo_dir.display())])
        .args(&["-v", "/cargo/bin"]) // Prevent `bin` from being mounted inside the Docker container.
        .args(&["-v", &format!("{}:/project:ro", root.display())])
        .args(&["-v", &format!("{}:/rust:ro", sysroot.display())])
        .args(&["-v", &format!("{}:/target", target_dir.display())])
        .args(&["-w", "/project"])
        .args(&["-it", &image(toml, target)?])
        .args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)])
        .run_and_get_status(verbose)
}

fn image(toml: Option<&Toml>, target: &Target) -> Result<String> {
    if let Some(toml) = toml {
        if let Some(image) = toml.image(target)?.map(|s| s.to_owned()) {
            return Ok(image)
        }
    }

    let version = env!("CARGO_PKG_VERSION");
    let tag = if version.ends_with("-dev") {
        Cow::from("latest")
    } else {
        Cow::from(format!("v{}", version))
    };
    let triple = target.triple();
    if !DOCKER_IMAGES.contains(&triple) {
        bail!("cross does not provide docker image for {} target, \
               specify a custom image in Cross.toml", triple);
    }
    Ok(format!("japaric/{}:{}", target.triple(), tag))
}
