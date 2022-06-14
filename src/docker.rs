use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::{env, fs};

use crate::cargo::CargoMetadata;
use crate::extensions::{CommandExt, SafeCommand};
use crate::file::write_file;
use crate::id;
use crate::{errors::*, file};
use crate::{Config, Target};
use atty::Stream;
use eyre::bail;

pub const CROSS_IMAGE: &str = "ghcr.io/cross-rs";
const DOCKER_IMAGES: &[&str] = &include!(concat!(env!("OUT_DIR"), "/docker-images.rs"));
const DOCKER: &str = "docker";
const PODMAN: &str = "podman";
// secured profile based off the docker documentation for denied syscalls:
// https://docs.docker.com/engine/security/seccomp/#significant-syscalls-blocked-by-the-default-profile
// note that we've allow listed `clone` and `clone3`, which is necessary
// to fork the process, and which podman allows by default.
const SECCOMP: &str = include_str!("seccomp.json");

#[derive(Debug, PartialEq, Eq)]
enum EngineType {
    Docker,
    Podman,
    Other,
}

// determine if the container engine is docker. this fixes issues with
// any aliases (#530), and doesn't fail if an executable suffix exists.
fn get_engine_type(ce: &Path, verbose: bool) -> Result<EngineType> {
    let stdout = Command::new(ce)
        .arg("--help")
        .run_and_get_stdout(verbose)?
        .to_lowercase();

    if stdout.contains("podman") {
        Ok(EngineType::Podman)
    } else if stdout.contains("docker") && !stdout.contains("emulate") {
        Ok(EngineType::Docker)
    } else {
        Ok(EngineType::Other)
    }
}

pub fn get_container_engine() -> Result<PathBuf, which::Error> {
    if let Ok(ce) = env::var("CROSS_CONTAINER_ENGINE") {
        which::which(ce)
    } else {
        which::which(DOCKER).or_else(|_| which::which(PODMAN))
    }
}

pub fn docker_command(engine: &Path, subcommand: &str) -> Result<Command> {
    let mut command = Command::new(engine);
    command.arg(subcommand);
    command.args(&["--userns", "host"]);
    Ok(command)
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

    let engine = get_container_engine()?;
    docker_command(&engine, "run")?
        .arg("--privileged")
        .arg("--rm")
        .arg("ubuntu:16.04")
        .args(&["sh", "-c", cmd])
        .run(verbose)
}

fn validate_env_var(var: &str) -> Result<(&str, Option<&str>)> {
    let (key, value) = match var.split_once('=') {
        Some((key, value)) => (key, Some(value)),
        _ => (var, None),
    };

    if key == "CROSS_RUNNER" {
        bail!("CROSS_RUNNER environment variable name is reserved and cannot be pass through");
    }

    Ok((key, value))
}

fn parse_docker_opts(value: &str) -> Result<Vec<String>> {
    shell_words::split(value).wrap_err_with(|| format!("could not parse docker opts of {}", value))
}

#[allow(unused_variables)]
pub fn mount(cmd: &mut Command, val: &Path, verbose: bool) -> Result<PathBuf> {
    let host_path = file::canonicalize(&val)
        .wrap_err_with(|| format!("when canonicalizing path `{}`", val.display()))?;
    let mount_path: PathBuf;
    #[cfg(target_os = "windows")]
    {
        // On Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
        mount_path = wslpath(&host_path, verbose)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        mount_path = host_path.clone();
    }
    cmd.args(&[
        "-v",
        &format!("{}:{}", host_path.display(), mount_path.display()),
    ]);
    Ok(mount_path)
}

#[allow(clippy::too_many_arguments)] // TODO: refactor
pub fn run(
    target: &Target,
    args: &[String],
    metadata: &CargoMetadata,
    config: &Config,
    uses_xargo: bool,
    sysroot: &Path,
    verbose: bool,
    docker_in_docker: bool,
    cwd: &Path,
) -> Result<ExitStatus> {
    let engine = get_container_engine()
        .map_err(|_| eyre::eyre!("no container engine found"))
        .with_suggestion(|| "is docker or podman installed?")?;
    let engine_type = get_engine_type(&engine, verbose)?;

    let mount_finder = if docker_in_docker {
        MountFinder::new(docker_read_mount_paths(&engine)?)
    } else {
        MountFinder::default()
    };

    let home_dir = home::home_dir().ok_or_else(|| eyre::eyre!("could not find home directory"))?;
    let cargo_dir = home::cargo_home()?;
    let xargo_dir = env::var_os("XARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir.join(".xargo"));
    let nix_store_dir = env::var_os("NIX_STORE").map(PathBuf::from);
    let target_dir = &metadata.target_directory;

    // create the directories we are going to mount before we mount them,
    // otherwise `docker` will create them but they will be owned by `root`
    fs::create_dir(&target_dir).ok();
    fs::create_dir(&cargo_dir).ok();
    fs::create_dir(&xargo_dir).ok();

    // update paths to the host mounts path.
    let cargo_dir = mount_finder.find_mount_path(cargo_dir);
    let xargo_dir = mount_finder.find_mount_path(xargo_dir);
    let target_dir = mount_finder.find_mount_path(target_dir);
    // root is either workspace_root, or, if we're outside the workspace root, the current directory
    let host_root = mount_finder.find_mount_path(if metadata.workspace_root.starts_with(cwd) {
        cwd
    } else {
        &metadata.workspace_root
    });
    let mount_root: PathBuf;
    #[cfg(target_os = "windows")]
    {
        // On Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
        mount_root = wslpath(&host_root, verbose)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        mount_root = mount_finder.find_mount_path(host_root.clone());
    }
    let mount_cwd: PathBuf;
    #[cfg(target_os = "windows")]
    {
        // On Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
        mount_cwd = wslpath(cwd, verbose)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        mount_cwd = mount_finder.find_mount_path(cwd);
    }
    let sysroot = mount_finder.find_mount_path(sysroot);

    let mut cmd = if uses_xargo {
        SafeCommand::new("xargo")
    } else {
        SafeCommand::new("cargo")
    };

    cmd.args(args);

    let runner = config.runner(target)?;

    let mut docker = docker_command(&engine, "run")?;

    for ref var in config.env_passthrough(target)? {
        validate_env_var(var)?;

        // Only specifying the environment variable name in the "-e"
        // flag forwards the value from the parent shell
        docker.args(&["-e", var]);
    }
    let mut mount_volumes = false;
    // FIXME(emilgardis 2022-04-07): This is a fallback so that if it's hard for us to do mounting logic, make it simple(r)
    // Preferably we would not have to do this.
    if cwd.strip_prefix(&metadata.workspace_root).is_err() {
        mount_volumes = true;
    }

    for ref var in config.env_volumes(target)? {
        let (var, value) = validate_env_var(var)?;
        let value = match value {
            Some(v) => Ok(v.to_string()),
            None => env::var(var),
        };

        if let Ok(val) = value {
            let mount_path = mount(&mut docker, val.as_ref(), verbose)?;
            docker.args(&["-e", &format!("{}={}", var, mount_path.display())]);
            mount_volumes = true;
        }
    }

    for path in metadata.path_dependencies() {
        mount(&mut docker, path, verbose)?;
        mount_volumes = true;
    }

    docker.args(&["-e", "PKG_CONFIG_ALLOW_CROSS=1"]);

    docker.arg("--rm");

    // docker uses seccomp now on all installations
    if target.needs_docker_seccomp() {
        let seccomp = if engine_type == EngineType::Docker && cfg!(target_os = "windows") {
            // docker on windows fails due to a bug in reading the profile
            // https://github.com/docker/for-win/issues/12760
            "unconfined".to_string()
        } else {
            #[allow(unused_mut)] // target_os = "windows"
            let mut path = env::current_dir()
                .wrap_err("couldn't get current directory")?
                .canonicalize()
                .wrap_err_with(|| "when canonicalizing current_dir".to_string())?
                .join("target")
                .join(target.triple())
                .join("seccomp.json");
            if !path.exists() {
                write_file(&path, false)?.write_all(SECCOMP.as_bytes())?;
            }
            #[cfg(target_os = "windows")]
            if engine_type == EngineType::Podman {
                // podman weirdly expects a WSL path here, and fails otherwise
                path = wslpath(&path, verbose)?;
            }
            path.display().to_string()
        };

        docker.args(&["--security-opt", &format!("seccomp={}", seccomp)]);
    }

    // We need to specify the user for Docker, but not for Podman.
    if engine_type == EngineType::Docker {
        docker.args(&[
            "--user",
            &format!(
                "{}:{}",
                env::var("CROSS_CONTAINER_UID").unwrap_or_else(|_| id::user().to_string()),
                env::var("CROSS_CONTAINER_GID").unwrap_or_else(|_| id::group().to_string()),
            ),
        ]);
    }

    docker
        .args(&["-e", "XARGO_HOME=/xargo"])
        .args(&["-e", "CARGO_HOME=/cargo"])
        .args(&["-e", "CARGO_TARGET_DIR=/target"]);

    if let Some(username) = id::username().unwrap() {
        docker.args(&["-e", &format!("USER={username}")]);
    }

    if let Ok(value) = env::var("QEMU_STRACE") {
        docker.args(&["-e", &format!("QEMU_STRACE={value}")]);
    }

    if let Ok(value) = env::var("CROSS_DEBUG") {
        docker.args(&["-e", &format!("CROSS_DEBUG={value}")]);
    }

    if let Ok(value) = env::var("CROSS_CONTAINER_OPTS") {
        if env::var("DOCKER_OPTS").is_ok() {
            eprintln!("Warning: using both `CROSS_CONTAINER_OPTS` and `DOCKER_OPTS`.");
        }
        docker.args(&parse_docker_opts(&value)?);
    } else if let Ok(value) = env::var("DOCKER_OPTS") {
        // FIXME: remove this when we deprecate DOCKER_OPTS.
        docker.args(&parse_docker_opts(&value)?);
    };

    docker
        .args(&[
            "-e",
            &format!("CROSS_RUNNER={}", runner.unwrap_or_default()),
        ])
        .args(&["-v", &format!("{}:/xargo:Z", xargo_dir.display())])
        .args(&["-v", &format!("{}:/cargo:Z", cargo_dir.display())])
        // Prevent `bin` from being mounted inside the Docker container.
        .args(&["-v", "/cargo/bin"]);
    if mount_volumes {
        docker.args(&[
            "-v",
            &format!("{}:{}:Z", host_root.display(), mount_root.display()),
        ]);
    } else {
        docker.args(&["-v", &format!("{}:/project:Z", host_root.display())]);
    }
    docker
        .args(&["-v", &format!("{}:/rust:Z,ro", sysroot.display())])
        .args(&["-v", &format!("{}:/target:Z", target_dir.display())]);

    if mount_volumes {
        docker.args(&["-w".as_ref(), mount_cwd.as_os_str()]);
    } else if mount_cwd == metadata.workspace_root {
        docker.args(&["-w", "/project"]);
    } else {
        // We do this to avoid clashes with path separators. Windows uses `\` as a path separator on Path::join
        let cwd = &cwd;
        let working_dir = Path::new("project").join(cwd.strip_prefix(&metadata.workspace_root)?);
        // No [T].join for OsStr
        let mut mount_wd = std::ffi::OsString::new();
        for part in working_dir.iter() {
            mount_wd.push("/");
            mount_wd.push(part);
        }
        docker.args(&["-w".as_ref(), mount_wd.as_os_str()]);
    }

    // When running inside NixOS or using Nix packaging we need to add the Nix
    // Store to the running container so it can load the needed binaries.
    if let Some(nix_store) = nix_store_dir {
        docker.args(&[
            "-v",
            &format!("{}:{}:Z", nix_store.display(), nix_store.display()),
        ]);
    }

    if atty::is(Stream::Stdin) {
        docker.arg("-i");
        if atty::is(Stream::Stdout) && atty::is(Stream::Stderr) {
            docker.arg("-t");
        }
    }

    docker
        .arg(&image(config, target)?)
        .args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)])
        .run_and_get_status(verbose)
}

pub fn image(config: &Config, target: &Target) -> Result<String> {
    if let Some(image) = config.image(target)? {
        return Ok(image);
    }

    if !DOCKER_IMAGES.contains(&target.triple()) {
        bail!(
            "`cross` does not provide a Docker image for target {target}, \
               specify a custom image in `Cross.toml`."
        );
    }

    let version = if include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt")).is_empty() {
        env!("CARGO_PKG_VERSION")
    } else {
        "main"
    };

    Ok(format!("{CROSS_IMAGE}/{target}:{version}"))
}

#[cfg(target_os = "windows")]
fn wslpath(path: &Path, verbose: bool) -> Result<PathBuf> {
    let wslpath = which::which("wsl.exe")
        .map_err(|_| eyre::eyre!("could not find wsl.exe"))
        .warning("usage of `env.volumes` requires WSL on Windows")
        .suggestion("is WSL installed on the host?")?;

    Command::new(wslpath)
        .arg("-e")
        .arg("wslpath")
        .arg("-a")
        .arg(path)
        .run_and_get_stdout(verbose)
        .wrap_err_with(|| {
            format!(
                "could not get linux compatible path for `{}`",
                path.display()
            )
        })
        .map(|s| s.trim().into())
}

fn docker_read_mount_paths(engine: &Path) -> Result<Vec<MountDetail>> {
    let hostname = env::var("HOSTNAME").wrap_err("HOSTNAME environment variable not found")?;

    let mut docker: Command = {
        let mut command = docker_command(engine, "inspect")?;
        command.arg(hostname);
        command
    };

    let output = docker.run_and_get_stdout(false)?;
    let info = serde_json::from_str(&output).wrap_err("failed to parse docker inspect output")?;
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
        .ok_or_else(|| eyre::eyre!("no driver name found"))?;

    if driver_name == "overlay2" {
        let path = info
            .pointer("/0/GraphDriver/Data/MergedDir")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("No merge directory found"))?;

        Ok(MountDetail {
            source: PathBuf::from(&path),
            destination: PathBuf::from("/"),
        })
    } else {
        eyre::bail!("want driver overlay2, got {driver_name}")
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
        .unwrap_or_else(Vec::new)
}

#[derive(Debug, Default)]
struct MountFinder {
    mounts: Vec<MountDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    fn find_mount_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();

        for info in &self.mounts {
            if let Ok(stripped) = path.strip_prefix(&info.destination) {
                return info.source.join(stripped);
            }
        }

        path.to_path_buf()
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
                finder.find_mount_path("/test/path"),
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
                finder.find_mount_path("/project/target/test")
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
                finder.find_mount_path("/container/path")
            );
            assert_eq!(
                PathBuf::from("/home/project/path"),
                finder.find_mount_path("/project")
            );
            assert_eq!(
                PathBuf::from("/home/project/path/target"),
                finder.find_mount_path("/project/target")
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
