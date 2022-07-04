use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use super::custom::Dockerfile;
use super::engine::*;
use crate::cargo::{cargo_metadata_with_args, CargoMetadata};
use crate::config::{bool_from_envvar, Config};
use crate::errors::*;
use crate::extensions::{CommandExt, SafeCommand};
use crate::file::{self, write_file, PathExt, ToUtf8};
use crate::id;
use crate::rustc::{self, VersionMetaExt};
use crate::shell::{self, MessageInfo, Verbosity};
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
    pub mount_root: String,
    pub mount_cwd: String,
    pub sysroot: PathBuf,
}

impl Directories {
    pub fn create(
        mount_finder: &MountFinder,
        metadata: &CargoMetadata,
        cwd: &Path,
        sysroot: &Path,
    ) -> Result<Self> {
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
        // cargo builds all intermediate directories, but fails
        // if it has other issues (such as permission errors).
        fs::create_dir_all(&cargo)?;
        fs::create_dir_all(&xargo)?;
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
        let mount_root: String;
        #[cfg(target_os = "windows")]
        {
            // On Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
            mount_root = host_root.as_wslpath()?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            // NOTE: host root has already found the mount path
            mount_root = host_root.to_utf8()?.to_string();
        }
        let mount_cwd = mount_finder.find_path(cwd, false)?;
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
    msg_info: MessageInfo,
) -> Result<(Target, CargoMetadata, Directories)> {
    let target_list = rustc::target_list((msg_info.color_choice, Verbosity::Quiet).into())?;
    let target = Target::from(target, &target_list);
    let metadata = cargo_metadata_with_args(None, None, msg_info)?
        .ok_or(eyre::eyre!("unable to get project metadata"))?;
    let cwd = std::env::current_dir()?;
    let host_meta = rustc::version_meta()?;
    let host = host_meta.host();

    let sysroot = rustc::get_sysroot(&host, &target, channel, msg_info)?.1;
    let mount_finder = MountFinder::create(engine, docker_in_docker)?;
    let dirs = Directories::create(&mount_finder, &metadata, &cwd, &sysroot)?;

    Ok((target, metadata, dirs))
}

/// Register binfmt interpreters
pub(crate) fn register(engine: &Engine, target: &Target, msg_info: MessageInfo) -> Result<()> {
    let cmd = if target.is_windows() {
        // https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html
        "mount binfmt_misc -t binfmt_misc /proc/sys/fs/binfmt_misc && \
            echo ':wine:M::MZ::/usr/bin/run-detectors:' > /proc/sys/fs/binfmt_misc/register"
    } else {
        "apt-get update && apt-get install --no-install-recommends --assume-yes \
            binfmt-support qemu-user-static"
    };

    let mut docker = subcommand(engine, "run");
    docker_userns(&mut docker);
    docker.arg("--privileged");
    docker.arg("--rm");
    docker.arg(UBUNTU_BASE);
    docker.args(&["sh", "-c", cmd]);

    docker.run(msg_info, false).map_err(Into::into)
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

fn add_cargo_configuration_envvars(docker: &mut Command) {
    let non_cargo_prefix = &[
        "http_proxy",
        "TERM",
        "RUSTDOCFLAGS",
        "RUSTFLAGS",
        "BROWSER",
        "HTTPS_PROXY",
        "HTTP_TIMEOUT",
        "https_proxy",
    ];
    let cargo_prefix_skip = &[
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET_DIR",
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTDOC",
    ];
    let is_cargo_passthrough = |key: &str| -> bool {
        non_cargo_prefix.contains(&key)
            || key.starts_with("CARGO_") && !cargo_prefix_skip.contains(&key)
    };

    // also need to accept any additional flags used to configure
    // cargo, but only pass what's actually present.
    for (key, _) in env::vars() {
        if is_cargo_passthrough(&key) {
            docker.args(&["-e", &key]);
        }
    }
}

// NOTE: host path must be canonical
pub(crate) fn mount(docker: &mut Command, host_path: &Path, prefix: &str) -> Result<String> {
    let mount_path = canonicalize_mount_path(host_path)?;
    docker.args(&[
        "-v",
        &format!("{}:{prefix}{}", host_path.to_utf8()?, mount_path),
    ]);
    Ok(mount_path)
}

pub(crate) fn docker_envvars(
    docker: &mut Command,
    config: &Config,
    target: &Target,
    msg_info: MessageInfo,
) -> Result<()> {
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
    add_cargo_configuration_envvars(docker);

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
            shell::warn(
                "using both `CROSS_CONTAINER_OPTS` and `DOCKER_OPTS`.",
                msg_info,
            )?;
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
        docker.args(&["-w", &dirs.mount_cwd]);
    } else if dirs.mount_cwd == metadata.workspace_root.to_utf8()? {
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
    mount_finder: &MountFinder,
    config: &Config,
    target: &Target,
    cwd: &Path,
    mount_cb: impl Fn(&mut Command, &Path) -> Result<String>,
    mut store_cb: impl FnMut((String, String)),
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
            let canonical_val = file::canonicalize(&val)?;
            let host_path = mount_finder.find_path(&canonical_val, true)?;
            let mount_path = mount_cb(docker, host_path.as_ref())?;
            docker.args(&["-e", &format!("{}={}", host_path, mount_path)]);
            store_cb((val, mount_path));
            mount_volumes = true;
        }
    }

    for path in metadata.path_dependencies() {
        let canonical_path = file::canonicalize(path)?;
        let host_path = mount_finder.find_path(&canonical_path, true)?;
        let mount_path = mount_cb(docker, host_path.as_ref())?;
        store_cb((path.to_utf8()?.to_string(), mount_path));
        mount_volumes = true;
    }

    Ok(mount_volumes)
}

pub(crate) fn canonicalize_mount_path(path: &Path) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        // On Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
        path.as_wslpath()
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_utf8().map(|p| p.to_string())
    }
}

pub(crate) fn user_id() -> String {
    env::var("CROSS_CONTAINER_UID").unwrap_or_else(|_| id::user().to_string())
}

pub(crate) fn group_id() -> String {
    env::var("CROSS_CONTAINER_GID").unwrap_or_else(|_| id::group().to_string())
}

pub(crate) fn docker_user_id(docker: &mut Command, engine_type: EngineType) {
    // by default, docker runs as root so we need to specify the user
    // so the resulting file permissions are for the current user.
    // since we can have rootless docker, we provide an override.
    let is_rootless = env::var("CROSS_ROOTLESS_CONTAINER_ENGINE")
        .ok()
        .and_then(|s| match s.as_ref() {
            "auto" => None,
            b => Some(bool_from_envvar(b)),
        })
        .unwrap_or_else(|| engine_type != EngineType::Docker);
    if !is_rootless {
        docker.args(&["--user", &format!("{}:{}", user_id(), group_id(),)]);
    }
}

pub(crate) fn docker_userns(docker: &mut Command) {
    let userns = match env::var("CROSS_CONTAINER_USER_NAMESPACE").ok().as_deref() {
        Some("none") => None,
        None | Some("auto") => Some("host".to_string()),
        Some(ns) => Some(ns.to_string()),
    };
    if let Some(ns) = userns {
        docker.args(&["--userns", &ns]);
    }
}

#[allow(unused_mut, clippy::let_and_return)]
pub(crate) fn docker_seccomp(
    docker: &mut Command,
    engine_type: EngineType,
    target: &Target,
    metadata: &CargoMetadata,
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
            let mut path_string = path.to_utf8()?.to_string();
            #[cfg(target_os = "windows")]
            if matches!(engine_type, EngineType::Podman | EngineType::PodmanRemote) {
                // podman weirdly expects a WSL path here, and fails otherwise
                path_string = path.as_wslpath()?;
            }
            path_string
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
    msg_info: MessageInfo,
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
                msg_info,
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
                    msg_info,
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

    let output = docker.run_and_get_stdout(Verbosity::Quiet.into())?;
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
pub struct MountFinder {
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

    pub fn create(engine: &Engine, docker_in_docker: bool) -> Result<MountFinder> {
        Ok(if docker_in_docker {
            MountFinder::new(docker_read_mount_paths(engine)?)
        } else {
            MountFinder::default()
        })
    }

    pub fn find_mount_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();

        for info in &self.mounts {
            if let Ok(stripped) = path.strip_prefix(&info.destination) {
                return info.source.join(stripped);
            }
        }

        path.to_path_buf()
    }

    #[allow(unused_variables, clippy::needless_return)]
    fn find_path(&self, path: &Path, host: bool) -> Result<String> {
        #[cfg(target_os = "windows")]
        {
            // On Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
            if host {
                return Ok(path.to_utf8()?.to_string());
            } else {
                return path.as_wslpath();
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            return Ok(self.find_mount_path(path).to_utf8()?.to_string());
        }
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
    use crate::id;

    #[test]
    fn test_docker_user_id() {
        let var = "CROSS_ROOTLESS_CONTAINER_ENGINE";
        let old = env::var(var);
        env::remove_var(var);

        let rootful = format!("\"engine\" \"--user\" \"{}:{}\"", id::user(), id::group());
        let rootless = "\"engine\"".to_string();

        let test = |engine, expected| {
            let mut cmd = Command::new("engine");
            docker_user_id(&mut cmd, engine);
            assert_eq!(expected, &format!("{cmd:?}"));
        };
        test(EngineType::Docker, &rootful);
        test(EngineType::Podman, &rootless);
        test(EngineType::PodmanRemote, &rootless);
        test(EngineType::Other, &rootless);

        env::set_var(var, "0");
        test(EngineType::Docker, &rootful);
        test(EngineType::Podman, &rootful);
        test(EngineType::PodmanRemote, &rootful);
        test(EngineType::Other, &rootful);

        env::set_var(var, "1");
        test(EngineType::Docker, &rootless);
        test(EngineType::Podman, &rootless);
        test(EngineType::PodmanRemote, &rootless);
        test(EngineType::Other, &rootless);

        env::set_var(var, "auto");
        test(EngineType::Docker, &rootful);
        test(EngineType::Podman, &rootless);
        test(EngineType::PodmanRemote, &rootless);
        test(EngineType::Other, &rootless);

        match old {
            Ok(v) => env::set_var(var, v),
            Err(_) => env::remove_var(var),
        }
    }

    #[test]
    fn test_docker_userns() {
        let var = "CROSS_CONTAINER_USER_NAMESPACE";
        let old = env::var(var);
        env::remove_var(var);

        let host = "\"engine\" \"--userns\" \"host\"".to_string();
        let custom = "\"engine\" \"--userns\" \"custom\"".to_string();
        let none = "\"engine\"".to_string();

        let test = |expected| {
            let mut cmd = Command::new("engine");
            docker_userns(&mut cmd);
            assert_eq!(expected, &format!("{cmd:?}"));
        };
        test(&host);

        env::set_var(var, "auto");
        test(&host);

        env::set_var(var, "none");
        test(&none);

        env::set_var(var, "host");
        test(&host);

        env::set_var(var, "custom");
        test(&custom);

        match old {
            Ok(v) => env::set_var(var, v),
            Err(_) => env::remove_var(var),
        }
    }

    mod directories {
        use super::*;
        use crate::cargo::cargo_metadata_with_args;
        use crate::temp;

        fn unset_env() -> Vec<(&'static str, Option<String>)> {
            let mut result = vec![];
            let envvars = ["CARGO_HOME", "XARGO_HOME", "NIX_STORE"];
            for var in envvars {
                result.push((var, env::var(var).ok()));
                env::remove_var(var);
            }

            result
        }

        fn reset_env(vars: Vec<(&'static str, Option<String>)>) {
            for (var, value) in vars {
                if let Some(value) = value {
                    env::set_var(var, value);
                }
            }
        }

        fn create_engine(msg_info: MessageInfo) -> Result<Engine> {
            Engine::from_path(get_container_engine()?, Some(false), msg_info)
        }

        fn cargo_metadata(subdir: bool, msg_info: MessageInfo) -> Result<CargoMetadata> {
            let mut metadata = cargo_metadata_with_args(
                Some(Path::new(env!("CARGO_MANIFEST_DIR"))),
                None,
                msg_info,
            )?
            .ok_or_else(|| eyre::eyre!("could not find cross workspace"))?;

            let root = match subdir {
                true => get_cwd()?.join("member"),
                false => get_cwd()?.parent().unwrap().to_path_buf(),
            };
            fs::create_dir_all(&root)?;
            metadata.workspace_root = root;
            metadata.target_directory = metadata.workspace_root.join("target");

            Ok(metadata)
        }

        fn home() -> Result<PathBuf> {
            home::home_dir().ok_or_else(|| eyre::eyre!("could not find home directory"))
        }

        fn get_cwd() -> Result<PathBuf> {
            // we need this directory to exist for Windows
            let path = temp::dir()?.join("Documents").join("package");
            fs::create_dir_all(&path)?;
            Ok(path)
        }

        fn get_sysroot() -> Result<PathBuf> {
            Ok(home()?
                .join(".rustup")
                .join("toolchains")
                .join("stable-x86_64-unknown-linux-gnu"))
        }

        fn get_directories(
            metadata: &CargoMetadata,
            mount_finder: &MountFinder,
        ) -> Result<Directories> {
            let cwd = get_cwd()?;
            let sysroot = get_sysroot()?;
            Directories::create(mount_finder, metadata, &cwd, &sysroot)
        }

        fn path_to_posix(path: &Path) -> Result<String> {
            #[cfg(target_os = "windows")]
            {
                path.as_wslpath()
            }
            #[cfg(not(target_os = "windows"))]
            {
                path.as_posix()
            }
        }

        #[track_caller]
        fn paths_equal(x: &Path, y: &Path) -> Result<()> {
            assert_eq!(path_to_posix(x)?, path_to_posix(y)?);
            Ok(())
        }

        #[test]
        fn test_host() -> Result<()> {
            let vars = unset_env();
            let mount_finder = MountFinder::new(vec![]);
            let metadata = cargo_metadata(false, MessageInfo::default())?;
            let directories = get_directories(&metadata, &mount_finder)?;
            paths_equal(&directories.cargo, &home()?.join(".cargo"))?;
            paths_equal(&directories.xargo, &home()?.join(".xargo"))?;
            paths_equal(&directories.host_root, &metadata.workspace_root)?;
            assert_eq!(
                &directories.mount_root,
                &path_to_posix(&metadata.workspace_root)?
            );
            assert_eq!(&directories.mount_cwd, &path_to_posix(&get_cwd()?)?);

            reset_env(vars);
            Ok(())
        }

        #[test]
        #[cfg_attr(not(target_os = "linux"), ignore)]
        fn test_docker_in_docker() -> Result<()> {
            let vars = unset_env();

            let engine = create_engine(MessageInfo::default());
            let hostname = env::var("HOSTNAME");
            if engine.is_err() || hostname.is_err() {
                eprintln!("could not get container engine or no hostname found");
                reset_env(vars);
                return Ok(());
            }
            let engine = engine.unwrap();
            let hostname = hostname.unwrap();
            let output = subcommand(&engine, "inspect")
                .arg(hostname)
                .run_and_get_output(MessageInfo::default())?;
            if !output.status.success() {
                eprintln!("inspect failed");
                reset_env(vars);
                return Ok(());
            }

            let mount_finder = MountFinder::create(&engine, true)?;
            let metadata = cargo_metadata(true, MessageInfo::default())?;
            let directories = get_directories(&metadata, &mount_finder)?;
            let mount_finder = MountFinder::new(docker_read_mount_paths(&engine)?);
            let mount_path = |p| mount_finder.find_mount_path(p);

            paths_equal(&directories.cargo, &mount_path(home()?.join(".cargo")))?;
            paths_equal(&directories.xargo, &mount_path(home()?.join(".xargo")))?;
            paths_equal(&directories.host_root, &mount_path(get_cwd()?))?;
            assert_eq!(
                &directories.mount_root,
                &path_to_posix(&mount_path(get_cwd()?))?
            );
            assert_eq!(
                &directories.mount_cwd,
                &path_to_posix(&mount_path(get_cwd()?))?
            );

            reset_env(vars);
            Ok(())
        }
    }

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
