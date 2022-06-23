use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use super::custom::Dockerfile;
use super::engine::*;
use crate::cargo::{cargo_metadata_with_args, CargoMetadata};
use crate::config::Config;
use crate::errors::*;
use crate::extensions::{CommandExt, SafeCommand};
use crate::file::{self, write_file, PathExt, ToUtf8};
use crate::id;
use crate::rustc::{self, VersionMetaExt};
use crate::Target;

pub use super::custom::CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX;

pub const CROSS_IMAGE: &str = "ghcr.io/cross-rs";
// note: this is the most common base image for our images
pub const UBUNTU_BASE: &str = "ubuntu:16.04";
const DOCKER_IMAGES: &[&str] = &include!(concat!(env!("OUT_DIR"), "/docker-images.rs"));

// secured profile based off the docker documentation for denied syscalls:
// https://docs.docker.com/engine/security/seccomp/#significant-syscalls-blocked-by-the-default-profile
// note that we've allow listed `clone` and `clone3`, which is necessary
// to fork the process, and which podman allows by default.
pub(crate) const SECCOMP: &str = include_str!("seccomp.json");

#[derive(Debug)]
pub struct Directories {
    pub cargo: PathBuf,
    pub xargo: PathBuf,
    pub target: PathBuf,
    pub nix_store: Option<PathBuf>,
    pub host_root: PathBuf,
    // both mount fields are WSL paths on windows: they already are POSIX paths
    pub mount_root: PathBuf,
    pub mount_cwd: PathBuf,
    pub sysroot: PathBuf,
}

impl Directories {
    #[allow(unused_variables)]
    pub fn create(
        engine: &Engine,
        metadata: &CargoMetadata,
        cwd: &Path,
        sysroot: &Path,
        docker_in_docker: bool,
        verbose: bool,
    ) -> Result<Self> {
        let mount_finder = if docker_in_docker {
            MountFinder::new(docker_read_mount_paths(engine)?)
        } else {
            MountFinder::default()
        };
        let home_dir =
            home::home_dir().ok_or_else(|| eyre::eyre!("could not find home directory"))?;
        let cargo = home::cargo_home()?;
        let xargo = env::var_os("XARGO_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".xargo"));
        let nix_store = env::var_os("NIX_STORE").map(PathBuf::from);
        let target = &metadata.target_directory;

        // create the directories we are going to mount before we mount them,
        // otherwise `docker` will create them but they will be owned by `root`
        fs::create_dir(&cargo).ok();
        fs::create_dir(&xargo).ok();
        create_target_dir(target)?;

        let cargo = mount_finder.find_mount_path(cargo);
        let xargo = mount_finder.find_mount_path(xargo);
        let target = mount_finder.find_mount_path(target);

        // root is either workspace_root, or, if we're outside the workspace root, the current directory
        let host_root = mount_finder.find_mount_path(if metadata.workspace_root.starts_with(cwd) {
            cwd
        } else {
            &metadata.workspace_root
        });

        // root is either workspace_root, or, if we're outside the workspace root, the current directory
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

        Ok(Directories {
            cargo,
            xargo,
            target,
            nix_store,
            host_root,
            mount_root,
            mount_cwd,
            sysroot,
        })
    }
}

const CACHEDIR_TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by cross.
# For information about cache directory tags see https://bford.info/cachedir/";

fn create_target_dir(path: &Path) -> Result<()> {
    // cargo creates all paths to the target directory, and writes
    // a cache dir tag only if the path doesn't previously exist.
    if !path.exists() {
        fs::create_dir_all(&path)?;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path.join("CACHEDIR.TAG"))?
            .write_all(CACHEDIR_TAG.as_bytes())?;
    }
    Ok(())
}

pub fn command(engine: &Engine) -> Command {
    let mut command = Command::new(&engine.path);
    if engine.needs_remote() {
        // if we're using podman and not podman-remote, need `--remote`.
        command.arg("--remote");
    }
    command
}

pub fn subcommand(engine: &Engine, subcommand: &str) -> Command {
    let mut command = command(engine);
    command.arg(subcommand);
    command
}

pub fn get_package_info(
    engine: &Engine,
    target: &str,
    channel: Option<&str>,
    docker_in_docker: bool,
    verbose: bool,
) -> Result<(Target, CargoMetadata, Directories)> {
    let target_list = rustc::target_list(false)?;
    let target = Target::from(target, &target_list);
    let metadata = cargo_metadata_with_args(None, None, verbose)?
        .ok_or(eyre::eyre!("unable to get project metadata"))?;
    let cwd = std::env::current_dir()?;
    let host_meta = rustc::version_meta()?;
    let host = host_meta.host();
    let sysroot = rustc::get_sysroot(&host, &target, channel, verbose)?.1;
    let dirs = Directories::create(engine, &metadata, &cwd, &sysroot, docker_in_docker, verbose)?;

    Ok((target, metadata, dirs))
}

/// Register binfmt interpreters
pub(crate) fn register(engine: &Engine, target: &Target, verbose: bool) -> Result<()> {
    let cmd = if target.is_windows() {
        // https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html
        "mount binfmt_misc -t binfmt_misc /proc/sys/fs/binfmt_misc && \
            echo ':wine:M::MZ::/usr/bin/run-detectors:' > /proc/sys/fs/binfmt_misc/register"
    } else {
        "apt-get update && apt-get install --no-install-recommends --assume-yes \
            binfmt-support qemu-user-static"
    };

    subcommand(engine, "run")
        .args(&["--userns", "host"])
        .arg("--privileged")
        .arg("--rm")
        .arg(UBUNTU_BASE)
        .args(&["sh", "-c", cmd])
        .run(verbose, false)
        .map_err(Into::into)
}

fn validate_env_var(var: &str) -> Result<(&str, Option<&str>)> {
    let (key, value) = match var.split_once('=') {
        Some((key, value)) => (key, Some(value)),
        _ => (var, None),
    };

    if key == "CROSS_RUNNER" {
        eyre::bail!(
            "CROSS_RUNNER environment variable name is reserved and cannot be pass through"
        );
    }

    Ok((key, value))
}

pub fn parse_docker_opts(value: &str) -> Result<Vec<String>> {
    shell_words::split(value).wrap_err_with(|| format!("could not parse docker opts of {}", value))
}

pub(crate) fn cargo_safe_command(uses_xargo: bool) -> SafeCommand {
    if uses_xargo {
        SafeCommand::new("xargo")
    } else {
        SafeCommand::new("cargo")
    }
}

pub(crate) fn mount(
    docker: &mut Command,
    val: &Path,
    prefix: &str,
    verbose: bool,
) -> Result<PathBuf> {
    let host_path = file::canonicalize(val)?;
    let mount_path = canonicalize_mount_path(&host_path, verbose)?;
    docker.args(&[
        "-v",
        &format!("{}:{prefix}{}", host_path.to_utf8()?, mount_path.to_utf8()?),
    ]);
    Ok(mount_path)
}

pub(crate) fn docker_envvars(docker: &mut Command, config: &Config, target: &Target) -> Result<()> {
    for ref var in config.env_passthrough(target)?.unwrap_or_default() {
        validate_env_var(var)?;

        // Only specifying the environment variable name in the "-e"
        // flag forwards the value from the parent shell
        docker.args(&["-e", var]);
    }

    let runner = config.runner(target)?;
    let cross_runner = format!("CROSS_RUNNER={}", runner.unwrap_or_default());
    docker
        .args(&["-e", "PKG_CONFIG_ALLOW_CROSS=1"])
        .args(&["-e", "XARGO_HOME=/xargo"])
        .args(&["-e", "CARGO_HOME=/cargo"])
        .args(&["-e", "CARGO_TARGET_DIR=/target"])
        .args(&["-e", &cross_runner]);

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

    Ok(())
}

pub(crate) fn docker_cwd(
    docker: &mut Command,
    metadata: &CargoMetadata,
    dirs: &Directories,
    cwd: &Path,
    mount_volumes: bool,
) -> Result<()> {
    if mount_volumes {
        docker.args(&["-w", dirs.mount_cwd.to_utf8()?]);
    } else if dirs.mount_cwd == metadata.workspace_root {
        docker.args(&["-w", "/project"]);
    } else {
        // We do this to avoid clashes with path separators. Windows uses `\` as a path separator on Path::join
        let cwd = &cwd;
        let working_dir = Path::new("/project").join(cwd.strip_prefix(&metadata.workspace_root)?);
        docker.args(&["-w", &working_dir.as_posix()?]);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)] // TODO: refactor
pub(crate) fn docker_mount(
    docker: &mut Command,
    metadata: &CargoMetadata,
    config: &Config,
    target: &Target,
    cwd: &Path,
    verbose: bool,
    mount_cb: impl Fn(&mut Command, &Path, bool) -> Result<PathBuf>,
    mut store_cb: impl FnMut((String, PathBuf)),
) -> Result<bool> {
    let mut mount_volumes = false;
    // FIXME(emilgardis 2022-04-07): This is a fallback so that if it's hard for us to do mounting logic, make it simple(r)
    // Preferably we would not have to do this.
    if cwd.strip_prefix(&metadata.workspace_root).is_err() {
        mount_volumes = true;
    }

    for ref var in config.env_volumes(target)?.unwrap_or_default() {
        let (var, value) = validate_env_var(var)?;
        let value = match value {
            Some(v) => Ok(v.to_string()),
            None => env::var(var),
        };

        if let Ok(val) = value {
            let mount_path = mount_cb(docker, val.as_ref(), verbose)?;
            docker.args(&["-e", &format!("{}={}", var, mount_path.to_utf8()?)]);
            store_cb((val, mount_path));
            mount_volumes = true;
        }
    }

    for path in metadata.path_dependencies() {
        let mount_path = mount_cb(docker, path, verbose)?;
        store_cb((path.to_utf8()?.to_string(), mount_path));
        mount_volumes = true;
    }

    Ok(mount_volumes)
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
        .wrap_err_with(|| format!("could not get linux compatible path for `{path:?}`"))
        .map(|s| s.trim().into())
}

#[allow(unused_variables)]
pub(crate) fn canonicalize_mount_path(path: &Path, verbose: bool) -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // On Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
        wslpath(path, verbose)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(path.to_path_buf())
    }
}

pub(crate) fn user_id() -> String {
    env::var("CROSS_CONTAINER_UID").unwrap_or_else(|_| id::user().to_string())
}

pub(crate) fn group_id() -> String {
    env::var("CROSS_CONTAINER_GID").unwrap_or_else(|_| id::group().to_string())
}

pub(crate) fn docker_user_id(docker: &mut Command, engine_type: EngineType) {
    // We need to specify the user for Docker, but not for Podman.
    if engine_type == EngineType::Docker {
        docker.args(&["--user", &format!("{}:{}", user_id(), group_id(),)]);
    }
}

#[allow(unused_variables)]
pub(crate) fn docker_seccomp(
    docker: &mut Command,
    engine_type: EngineType,
    target: &Target,
    metadata: &CargoMetadata,
    verbose: bool,
) -> Result<()> {
    // docker uses seccomp now on all installations
    if target.needs_docker_seccomp() {
        let seccomp = if engine_type == EngineType::Docker && cfg!(target_os = "windows") {
            // docker on windows fails due to a bug in reading the profile
            // https://github.com/docker/for-win/issues/12760
            "unconfined".to_string()
        } else {
            #[allow(unused_mut)] // target_os = "windows"
            let mut path = metadata
                .target_directory
                .join(target.triple())
                .join("seccomp.json");
            if !path.exists() {
                write_file(&path, false)?.write_all(SECCOMP.as_bytes())?;
            }
            #[cfg(target_os = "windows")]
            if matches!(engine_type, EngineType::Podman | EngineType::PodmanRemote) {
                // podman weirdly expects a WSL path here, and fails otherwise
                path = wslpath(&path, verbose)?;
            }
            path.to_utf8()?.to_string()
        };

        docker.args(&["--security-opt", &format!("seccomp={}", seccomp)]);
    }

    Ok(())
}

pub fn needs_custom_image(target: &Target, config: &Config) -> bool {
    config.dockerfile(target).unwrap_or_default().is_some()
        || !config
            .pre_build(target)
            .unwrap_or_default()
            .unwrap_or_default()
            .is_empty()
}

pub(crate) fn custom_image_build(
    target: &Target,
    config: &Config,
    metadata: &CargoMetadata,
    Directories { host_root, .. }: Directories,
    engine: &Engine,
    verbose: bool,
) -> Result<String> {
    let mut image = image_name(config, target)?;

    if let Some(path) = config.dockerfile(target)? {
        let context = config.dockerfile_context(target)?;
        let name = config.image(target)?;

        let build = Dockerfile::File {
            path: &path,
            context: context.as_deref(),
            name: name.as_deref(),
        };

        image = build
            .build(
                config,
                metadata,
                engine,
                &host_root,
                config.dockerfile_build_args(target)?.unwrap_or_default(),
                target,
                verbose,
            )
            .wrap_err("when building dockerfile")?;
    }
    let pre_build = config.pre_build(target)?;

    if let Some(pre_build) = pre_build {
        if !pre_build.is_empty() {
            let custom = Dockerfile::Custom {
                content: format!(
                    r#"
    FROM {image}
    ARG CROSS_DEB_ARCH=
    ARG CROSS_CMD
    RUN eval "${{CROSS_CMD}}""#
                ),
            };
            custom
                .build(
                    config,
                    metadata,
                    engine,
                    &host_root,
                    Some(("CROSS_CMD", pre_build.join("\n"))),
                    target,
                    verbose,
                )
                .wrap_err("when pre-building")
                .with_note(|| format!("CROSS_CMD={}", pre_build.join("\n")))?;
            image = custom.image_name(target, metadata)?;
        }
    }
    Ok(image)
}

pub(crate) fn image_name(config: &Config, target: &Target) -> Result<String> {
    if let Some(image) = config.image(target)? {
        return Ok(image);
    }

    if !DOCKER_IMAGES.contains(&target.triple()) {
        eyre::bail!(
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

fn docker_read_mount_paths(engine: &Engine) -> Result<Vec<MountDetail>> {
    let hostname = env::var("HOSTNAME").wrap_err("HOSTNAME environment variable not found")?;

    let mut docker: Command = {
        let mut command = subcommand(engine, "inspect");
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

fn path_digest(path: &Path) -> Result<const_sha1::Digest> {
    let buffer = const_sha1::ConstBuffer::from_slice(path.to_utf8()?.as_bytes());
    Ok(const_sha1::sha1(&buffer))
}

pub fn path_hash(path: &Path) -> Result<String> {
    Ok(path_digest(path)?
        .to_string()
        .get(..5)
        .expect("sha1 is expected to be at least 5 characters long")
        .to_string())
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
