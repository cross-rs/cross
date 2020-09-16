use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::{env, fs};

use atty::Stream;
use error_chain::bail;
use serde_json;

use crate::cargo::Root;
use crate::errors::*;
use crate::extensions::{CommandExt, SafeCommand};
use crate::id;
use crate::{Target, Toml};

const DOCKER_IMAGES: &[&str] = &include!(concat!(env!("OUT_DIR"), "/docker-images.rs"));
const DOCKER: &str = "docker";
const PODMAN: &str = "podman";

fn get_container_engine() -> Result<std::path::PathBuf> {
    which::which(DOCKER).or_else(|_| which::which(PODMAN)).map_err(|e| e.into())
}

pub fn docker_command(subcommand: &str) -> Result<Command> {
    if let Ok(ce) = get_container_engine() {
        let mut command = Command::new(ce);
        command.arg(subcommand);
        command.args(&["--userns", "host"]);
        Ok(command)
    } else {
        Err("no container engine found; install docker or podman".into())
    }
}

/// Register binfmt interpreters
pub fn register(target: &Target, verbose: bool) -> Result<()> {
    let cmd = if target.is_windows() {
        // https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html
        "mount binfmt_misc -t binfmt_misc /proc/sys/fs/binfmt_misc && \
            echo ':wine:M::MZ::/usr/bin/run-detectors:' > /proc/sys/fs/binfmt_misc/register"
    } else {
        "apt-get update && apt-get install --no-install-recommends --assume-yes \
            binfmt-support qemu-user-static"
    };

    docker_command("run")?
        .arg("--privileged")
        .arg("--rm")
        .arg("ubuntu:16.04")
        .args(&["sh", "-c", cmd])
        .run(verbose)
}

pub fn run(
    target: &Target,
    docker_image: Option<&str>,
    args: &[String],
    target_dir: &Option<PathBuf>,
    root: &Root,
    toml: Option<&Toml>,
    uses_xargo: bool,
    sysroot: &PathBuf,
    verbose: bool,
    docker_in_docker: bool,
) -> Result<ExitStatus> {
    let mount_finder = if docker_in_docker {
        MountFinder::new(docker_read_mount_paths()?)
    } else {
        MountFinder::default()
    };

    let root = root.path();
    let home_dir = home::home_dir().ok_or_else(|| "could not find home directory")?;
    let cargo_dir = home::cargo_home()?;
    let xargo_dir = env::var_os("XARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.join(".xargo"));
    let target_dir = target_dir.clone().unwrap_or_else(|| root.join("target"));

    // create the directories we are going to mount before we mount them,
    // otherwise `docker` will create them but they will be owned by `root`
    fs::create_dir(&target_dir).ok();
    fs::create_dir(&cargo_dir).ok();
    fs::create_dir(&xargo_dir).ok();

    // update paths to the host mounts path.
    let cargo_dir = mount_finder.find_mount_path(&cargo_dir);
    let xargo_dir = mount_finder.find_mount_path(&xargo_dir);
    let target_dir = mount_finder.find_mount_path(&target_dir);
    let mount_root = mount_finder.find_mount_path(&root);
    let sysroot = mount_finder.find_mount_path(&sysroot);

    let mut cmd = if uses_xargo {
        SafeCommand::new("xargo")
    } else {
        SafeCommand::new("cargo")
    };

    cmd.args(args);

    let runner = None;

    let mut docker = docker_command("run")?;

    if let Some(toml) = toml {
        let validate_env_var = |var: &str| -> Result<()> {
            if var.contains('=') {
                bail!("environment variable names must not contain the '=' character");
            }

            if var == "CROSS_RUNNER" {
                bail!(
                    "CROSS_RUNNER environment variable name is reserved and cannot be pass through"
                );
            }

            Ok(())
        };

        for var in toml.env_passthrough(target)? {
            validate_env_var(var)?;

            // Only specifying the environment variable name in the "-e"
            // flag forwards the value from the parent shell
            docker.args(&["-e", var]);
        }

        for var in toml.env_volumes(target)? {
            validate_env_var(var)?;

            if let Ok(val) = env::var(var) {
                let host_path = Path::new(&val).canonicalize()?;
                let mount_path = &host_path;
                docker.args(&["-v", &format!("{}:{}", host_path.display(), mount_path.display())]);
                docker.args(&["-e", &format!("{}={}", var, mount_path.display())]);
            }
        }
    }

    docker.args(&["-e", "PKG_CONFIG_ALLOW_CROSS=1"]);

    docker.arg("--rm");

    // We need to specify the user for Docker, but not for Podman.
    if let Ok(ce) = get_container_engine() {
        if ce.ends_with(DOCKER) {
            docker.args(&["--user", &format!("{}:{}", id::user(), id::group())]);
        }
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
        let opts: Vec<&str> = value.split(' ').collect();
        docker.args(&opts);
    }

    docker
        .args(&["-e", &format!("CROSS_RUNNER={}", runner.unwrap_or_else(String::new))])
        .args(&["-v", &format!("{}:/xargo:Z", xargo_dir.display())])
        .args(&["-v", &format!("{}:/cargo:Z", cargo_dir.display())])
        // Prevent `bin` from being mounted inside the Docker container.
        .args(&["-v", "/cargo/bin"])
        .args(&["-v", &format!("{}:/{}:Z", mount_root.display(), mount_root.display())])
        .args(&["-v", &format!("{}:/rust:Z,ro", sysroot.display())])
        .args(&["-v", &format!("{}:/target:Z", target_dir.display())])
        .args(&["-w", &mount_root.display().to_string()]);

    if atty::is(Stream::Stdin) {
        docker.arg("-i");
        if atty::is(Stream::Stdout) && atty::is(Stream::Stderr) {
            docker.arg("-t");
        }
    }

    let docker_image = docker_image
        .map(str::to_string)
        .ok_or(())
        .or_else(|_| image(toml, target))?;

    docker
        .arg(docker_image)
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

fn docker_read_mount_paths() -> Result<Vec<MountDetail>> {
    let hostname = if let Ok(v) = env::var("HOSTNAME") {
        Ok(v)
    } else {
        Err("HOSTNAME environment variable not found")
    }?;

    let docker_path = which::which(DOCKER)?;
    let mut docker: Command = {
        let mut command = Command::new(docker_path);
        command.arg("inspect");
        command.arg(hostname);
        command
    };

    let output = docker.run_and_get_stdout(false)?;
    let info = if let Ok(val) = serde_json::from_str(&output) {
        Ok(val)
    } else {
        Err("failed to parse docker inspect output")
    }?;

    dockerinfo_parse_mounts(&info)
}

fn dockerinfo_parse_mounts(info: &serde_json::Value) -> Result<Vec<MountDetail>> {
    let mut mounts = dockerinfo_parse_user_mounts(info);
    let root_info = dockerinfo_parse_root_mount_path(info)?;
    mounts.push(root_info);
    Ok(mounts)
}

fn dockerinfo_parse_root_mount_path(info: &serde_json::Value) -> Result<MountDetail> {
    let driver_name = info
        .pointer("/0/GraphDriver/Name")
        .and_then(|v| v.as_str())
        .ok_or("No driver name found")?;

    if driver_name == "overlay2" {
        let path = info
            .pointer("/0/GraphDriver/Data/MergedDir")
            .and_then(|v| v.as_str())
            .ok_or("No merge directory found")?;

        Ok(MountDetail {
            source: PathBuf::from(&path),
            destination: PathBuf::from("/"),
        })
    } else {
        Err(format!("want driver overlay2, got {}", driver_name).into())
    }
}

fn dockerinfo_parse_user_mounts(info: &serde_json::Value) -> Vec<MountDetail> {
    info.pointer("/0/Mounts")
        .and_then(|v| v.as_array())
        .map(|v| {
            let make_path = |v: &serde_json::Value| PathBuf::from(&v.as_str().unwrap());
            let mut mounts = vec![];
            for details in v {
                let source = make_path(&details["Source"]);
                let destination = make_path(&details["Destination"]);
                mounts.push(MountDetail {
                    source,
                    destination,
                });
            }
            mounts
        })
        .unwrap_or_else(|| Vec::new())
}

#[derive(Debug, Default)]
struct MountFinder {
    mounts: Vec<MountDetail>,
}

#[derive(Debug, Clone, PartialEq)]
struct MountDetail {
    source: PathBuf,
    destination: PathBuf,
}

impl MountFinder {
    fn new(mounts: Vec<MountDetail>) -> MountFinder {
        // sort by length (reverse), to give mounts with more path components a higher priority;
        let mut mounts = mounts;
        mounts.sort_by(|a, b| {
            let la = a.destination.as_os_str().len();
            let lb = b.destination.as_os_str().len();
            la.cmp(&lb).reverse()
        });
        MountFinder { mounts }
    }

    fn find_mount_path(&self, path: &Path) -> PathBuf {
        for info in &self.mounts {
            if let Ok(stripped) = path.strip_prefix(&info.destination) {
                return info.source.join(stripped);
            }
        }
        return path.to_path_buf();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod mount_finder {
        use super::*;

        #[test]
        fn test_default_finder_returns_original() {
            let finder = MountFinder::default();
            assert_eq!(
                PathBuf::from("/test/path"),
                finder.find_mount_path(&PathBuf::from("/test/path")),
            );
        }

        #[test]
        fn test_longest_destination_path_wins() {
            let finder = MountFinder::new(vec![
                MountDetail {
                    source: PathBuf::from("/project/path"),
                    destination: PathBuf::from("/project"),
                },
                MountDetail {
                    source: PathBuf::from("/target/path"),
                    destination: PathBuf::from("/project/target"),
                },
            ]);
            assert_eq!(
                PathBuf::from("/target/path/test"),
                finder.find_mount_path(&PathBuf::from("/project/target/test"))
            )
        }

        #[test]
        fn test_adjust_multiple_paths() {
            let finder = MountFinder::new(vec![
                MountDetail {
                    source: PathBuf::from("/var/lib/docker/overlay2/container-id/merged"),
                    destination: PathBuf::from("/"),
                },
                MountDetail {
                    source: PathBuf::from("/home/project/path"),
                    destination: PathBuf::from("/project"),
                },
            ]);
            assert_eq!(
                PathBuf::from("/var/lib/docker/overlay2/container-id/merged/container/path"),
                finder.find_mount_path(&PathBuf::from("/container/path"))
            );
            assert_eq!(
                PathBuf::from("/home/project/path"),
                finder.find_mount_path(&PathBuf::from("/project"))
            );
            assert_eq!(
                PathBuf::from("/home/project/path/target"),
                finder.find_mount_path(&PathBuf::from("/project/target"))
            );
        }
    }

    mod parse_docker_inspect {
        use super::*;
        use serde_json::json;

        #[test]
        fn test_parse_container_root() {
            let actual = dockerinfo_parse_root_mount_path(&json!([{
                "GraphDriver": {
                    "Data": {
                        "LowerDir": "/var/lib/docker/overlay2/f107af83b37bc0a182d3d2661f3d84684f0fffa1a243566b338a388d5e54bef4-init/diff:/var/lib/docker/overlay2/dfe81d459bbefada7aa897a9d05107a77145b0d4f918855f171ee85789ab04a0/diff:/var/lib/docker/overlay2/1f704696915c75cd081a33797ecc66513f9a7a3ffab42d01a3f17c12c8e2dc4c/diff:/var/lib/docker/overlay2/0a4f6cb88f4ace1471442f9053487a6392c90d2c6e206283d20976ba79b38a46/diff:/var/lib/docker/overlay2/1ee3464056f9cdc968fac8427b04e37ec96b108c5050812997fa83498f2499d1/diff:/var/lib/docker/overlay2/0ec5a47f1854c0f5cfe0e3f395b355b5a8bb10f6e622710ce95b96752625f874/diff:/var/lib/docker/overlay2/f24c8ad76303838b49043d17bf2423fe640836fd9562d387143e68004f8afba0/diff:/var/lib/docker/overlay2/462f89d5a0906805a6f2eec48880ed1e48256193ed506da95414448d435db2b7/diff",
                        "MergedDir": "/var/lib/docker/overlay2/f107af83b37bc0a182d3d2661f3d84684f0fffa1a243566b338a388d5e54bef4/merged",
                        "UpperDir": "/var/lib/docker/overlay2/f107af83b37bc0a182d3d2661f3d84684f0fffa1a243566b338a388d5e54bef4/diff",
                        "WorkDir": "/var/lib/docker/overlay2/f107af83b37bc0a182d3d2661f3d84684f0fffa1a243566b338a388d5e54bef4/work"
                    },
                    "Name": "overlay2"
                },
            }])).unwrap();
            let want = MountDetail {
                source: PathBuf::from("/var/lib/docker/overlay2/f107af83b37bc0a182d3d2661f3d84684f0fffa1a243566b338a388d5e54bef4/merged"),
                destination: PathBuf::from("/"),
            };
            assert_eq!(want, actual);
        }

        #[test]
        fn test_parse_empty_user_mounts() {
            let actual = dockerinfo_parse_user_mounts(&json!([{
                "Mounts": [],
            }]));
            assert_eq!(Vec::<MountDetail>::new(), actual);
        }

        #[test]
        fn test_parse_missing_user_moutns() {
            let actual = dockerinfo_parse_user_mounts(&json!([{
                "Id": "test",
            }]));
            assert_eq!(Vec::<MountDetail>::new(), actual);
        }
    }
}
