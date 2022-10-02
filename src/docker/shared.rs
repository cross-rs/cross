use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use super::custom::{Dockerfile, PreBuild};
use super::engine::*;
use super::image::PossibleImage;
use super::Image;
use super::PROVIDED_IMAGES;
use crate::cargo::{cargo_metadata_with_args, CargoMetadata};
use crate::config::{bool_from_envvar, Config};
use crate::errors::*;
use crate::extensions::{CommandExt, SafeCommand};
use crate::file::{self, write_file, PathExt, ToUtf8};
use crate::id;
use crate::rustc::QualifiedToolchain;
use crate::shell::{MessageInfo, Verbosity};
use crate::{CargoVariant, Target};

use rustc_version::Version as RustcVersion;

pub use super::custom::CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX;

pub const CROSS_IMAGE: &str = "ghcr.io/cross-rs";
// note: this is the most common base image for our images
pub const UBUNTU_BASE: &str = "ubuntu:20.04";

// secured profile based off the docker documentation for denied syscalls:
// https://docs.docker.com/engine/security/seccomp/#significant-syscalls-blocked-by-the-default-profile
// note that we've allow listed `clone` and `clone3`, which is necessary
// to fork the process, and which podman allows by default.
pub(crate) const SECCOMP: &str = include_str!("seccomp.json");

#[derive(Debug)]
pub struct DockerOptions {
    pub engine: Engine,
    pub target: Target,
    pub config: Config,
    pub image: Image,
    pub cargo_variant: CargoVariant,
    // not all toolchains will provide this
    pub rustc_version: Option<RustcVersion>,
}

impl DockerOptions {
    pub fn new(
        engine: Engine,
        target: Target,
        config: Config,
        image: Image,
        cargo_variant: CargoVariant,
        rustc_version: Option<RustcVersion>,
    ) -> DockerOptions {
        DockerOptions {
            engine,
            target,
            config,
            image,
            cargo_variant,
            rustc_version,
        }
    }

    #[must_use]
    pub fn in_docker(&self) -> bool {
        self.engine.in_docker
    }

    #[must_use]
    pub fn is_remote(&self) -> bool {
        self.engine.is_remote
    }

    #[must_use]
    pub fn needs_custom_image(&self) -> bool {
        self.config
            .dockerfile(&self.target)
            .unwrap_or_default()
            .is_some()
            || self
                .config
                .pre_build(&self.target)
                .unwrap_or_default()
                .is_some()
    }

    pub(crate) fn custom_image_build(
        &self,
        paths: &DockerPaths,
        msg_info: &mut MessageInfo,
    ) -> Result<String> {
        let mut image = self.image.clone();

        if let Some(path) = self.config.dockerfile(&self.target)? {
            let context = self.config.dockerfile_context(&self.target)?;

            let is_custom_image = self.config.image(&self.target)?.is_some();

            let build = Dockerfile::File {
                path: &path,
                context: context.as_deref(),
                name: if is_custom_image {
                    Some(&image.name)
                } else {
                    None
                },
                runs_with: &image.platform,
            };

            image.name = build
                .build(
                    self,
                    paths,
                    self.config
                        .dockerfile_build_args(&self.target)?
                        .unwrap_or_default(),
                    msg_info,
                )
                .wrap_err("when building dockerfile")?;
        }
        let pre_build = self.config.pre_build(&self.target)?;

        if let Some(pre_build) = pre_build {
            match pre_build {
                super::custom::PreBuild::Single {
                    line: pre_build_script,
                    env,
                } if !env
                    || !pre_build_script.contains('\n')
                        && paths.host_root().join(&pre_build_script).is_file() =>
                {
                    let custom = Dockerfile::Custom {
                        content: format!(
                            r#"
                FROM {image}
                ARG CROSS_DEB_ARCH=
                ARG CROSS_SCRIPT
                ARG CROSS_TARGET
                COPY $CROSS_SCRIPT /pre-build-script
                RUN chmod +x /pre-build-script
                RUN ./pre-build-script $CROSS_TARGET"#
                        ),
                        runs_with: &image.platform,
                    };

                    image.name = custom
                        .build(
                            self,
                            paths,
                            vec![
                                ("CROSS_SCRIPT", &*pre_build_script),
                                ("CROSS_TARGET", self.target.triple()),
                            ],
                            msg_info,
                        )
                        .wrap_err("when pre-building")
                        .with_note(|| format!("CROSS_SCRIPT={pre_build_script}"))
                        .with_note(|| format!("CROSS_TARGET={}", self.target))?;
                }
                this => {
                    let pre_build = match this {
                        PreBuild::Single { line, .. } => vec![line],
                        PreBuild::Lines(lines) => lines,
                    };
                    if !pre_build.is_empty() {
                        let custom = Dockerfile::Custom {
                            content: format!(
                                r#"
                FROM {image}
                ARG CROSS_DEB_ARCH=
                ARG CROSS_CMD
                RUN eval "${{CROSS_CMD}}""#
                            ),
                            runs_with: &image.platform,
                        };
                        image.name = custom
                            .build(
                                self,
                                paths,
                                Some(("CROSS_CMD", pre_build.join("\n"))),
                                msg_info,
                            )
                            .wrap_err("when pre-building")
                            .with_note(|| format!("CROSS_CMD={}", pre_build.join("\n")))?;
                    }
                }
            }
        }
        Ok(image.name.clone())
    }
}

#[derive(Debug)]
pub struct DockerPaths {
    pub mount_finder: MountFinder,
    pub metadata: CargoMetadata,
    pub cwd: PathBuf,
    pub directories: Directories,
}

impl DockerPaths {
    pub fn create(
        engine: &Engine,
        metadata: CargoMetadata,
        cwd: PathBuf,
        toolchain: QualifiedToolchain,
    ) -> Result<Self> {
        let mount_finder = MountFinder::create(engine)?;
        let (directories, metadata) =
            Directories::assemble(&mount_finder, metadata, &cwd, toolchain)?;
        Ok(Self {
            mount_finder,
            metadata,
            cwd,
            directories,
        })
    }

    pub fn get_sysroot(&self) -> &Path {
        self.directories.get_sysroot()
    }

    pub fn workspace_root(&self) -> &Path {
        &self.metadata.workspace_root
    }

    pub fn workspace_dependencies(&self) -> impl Iterator<Item = &Path> {
        self.metadata.path_dependencies()
    }

    pub fn workspace_from_cwd(&self) -> Result<&Path> {
        self.cwd
            .strip_prefix(self.workspace_root())
            .map_err(Into::into)
    }

    #[must_use]
    pub fn in_workspace(&self) -> bool {
        self.workspace_from_cwd().is_ok()
    }

    pub fn mount_cwd(&self) -> &str {
        &self.directories.mount_cwd
    }

    pub fn host_root(&self) -> &Path {
        &self.directories.host_root
    }
}

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
    pub toolchain: QualifiedToolchain,
    pub cargo_mount_path: String,
    pub xargo_mount_path: String,
    pub sysroot_mount_path: String,
}

impl Directories {
    pub fn assemble(
        mount_finder: &MountFinder,
        mut metadata: CargoMetadata,
        cwd: &Path,
        mut toolchain: QualifiedToolchain,
    ) -> Result<(Self, CargoMetadata)> {
        let home_dir =
            home::home_dir().ok_or_else(|| eyre::eyre!("could not find home directory"))?;
        let cargo = home::cargo_home()?;
        let xargo =
            env::var_os("XARGO_HOME").map_or_else(|| home_dir.join(".xargo"), PathBuf::from);
        // NIX_STORE_DIR is an override of NIX_STORE, which is the path in derivations.
        let nix_store = env::var_os("NIX_STORE_DIR")
            .or_else(|| env::var_os("NIX_STORE"))
            .map(PathBuf::from);
        let target = &metadata.target_directory;

        // create the directories we are going to mount before we mount them,
        // otherwise `docker` will create them but they will be owned by `root`
        // cargo builds all intermediate directories, but fails
        // if it has other issues (such as permission errors).
        file::create_dir_all(&cargo)?;
        file::create_dir_all(&xargo)?;
        if let Some(ref nix_store) = nix_store {
            file::create_dir_all(nix_store)?;
        }
        create_target_dir(target)?;

        // get our mount paths prior to canonicalizing them
        let cargo_mount_path = cargo.as_posix_absolute()?;
        let xargo_mount_path = xargo.as_posix_absolute()?;

        // now that we know the paths exist, canonicalize them. this avoids creating
        // directories after failed canonicalization into a shared directory.
        let cargo = file::canonicalize(&cargo)?;
        let xargo = file::canonicalize(&xargo)?;

        let default_nix_store = PathBuf::from("/nix/store");
        let nix_store = match nix_store {
            Some(store) if store.exists() => {
                let path = file::canonicalize(&store)?;
                Some(path)
            }
            Some(store) => {
                eyre::bail!("unable to find provided nix-store directory {store:?}");
            }
            None if cfg!(target_os = "linux") && default_nix_store.exists() => {
                Some(default_nix_store)
            }
            None => None,
        };

        let cargo = mount_finder.find_mount_path(cargo);
        let xargo = mount_finder.find_mount_path(xargo);
        metadata.target_directory = mount_finder.find_mount_path(target);

        // root is either workspace_root, or, if we're outside the workspace root, the current directory
        let host_root = mount_finder.find_mount_path(if metadata.workspace_root.starts_with(cwd) {
            cwd
        } else {
            &metadata.workspace_root
        });

        // on Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
        // NOTE: on unix, host root has already found the mount path
        let mount_root = host_root.as_posix_absolute()?;
        let mount_cwd = mount_finder.find_path(cwd, false)?;

        toolchain.set_sysroot(|p| mount_finder.find_mount_path(p));

        // canonicalize these once to avoid syscalls
        let sysroot_mount_path = toolchain.get_sysroot().as_posix_absolute()?;

        Ok((
            Directories {
                cargo,
                xargo,
                target: metadata.target_directory.clone(),
                nix_store,
                host_root,
                mount_root,
                mount_cwd,
                toolchain,
                cargo_mount_path,
                xargo_mount_path,
                sysroot_mount_path,
            },
            metadata,
        ))
    }

    pub fn get_sysroot(&self) -> &Path {
        self.toolchain.get_sysroot()
    }

    pub fn cargo_mount_path(&self) -> &str {
        &self.cargo_mount_path
    }

    pub fn xargo_mount_path(&self) -> &str {
        &self.xargo_mount_path
    }

    pub fn sysroot_mount_path(&self) -> &str {
        &self.sysroot_mount_path
    }

    pub fn cargo_mount_path_relative(&self) -> Result<String> {
        self.cargo_mount_path()
            .strip_prefix('/')
            .map(ToOwned::to_owned)
            .ok_or_else(|| eyre::eyre!("cargo directory must be relative to root"))
    }

    pub fn xargo_mount_path_relative(&self) -> Result<String> {
        self.xargo_mount_path()
            .strip_prefix('/')
            .map(ToOwned::to_owned)
            .ok_or_else(|| eyre::eyre!("xargo directory must be relative to root"))
    }

    pub fn sysroot_mount_path_relative(&self) -> Result<String> {
        self.sysroot_mount_path()
            .strip_prefix('/')
            .map(ToOwned::to_owned)
            .ok_or_else(|| eyre::eyre!("sysroot directory must be relative to root"))
    }
}

const CACHEDIR_TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by cross.
# For information about cache directory tags see https://bford.info/cachedir/";

fn create_target_dir(path: &Path) -> Result<()> {
    // cargo creates all paths to the target directory, and writes
    // a cache dir tag only if the path doesn't previously exist.
    if !path.exists() {
        file::create_dir_all(path)?;
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

pub fn subcommand(engine: &Engine, cmd: &str) -> Command {
    let mut command = command(engine);
    command.arg(cmd);
    command
}

pub fn get_package_info(
    engine: &Engine,
    toolchain: QualifiedToolchain,
    msg_info: &mut MessageInfo,
) -> Result<(Directories, CargoMetadata)> {
    let metadata = cargo_metadata_with_args(None, None, msg_info)?
        .ok_or(eyre::eyre!("unable to get project metadata"))?;
    let mount_finder = MountFinder::create(engine)?;
    let cwd = std::env::current_dir()?;
    Directories::assemble(&mount_finder, metadata, &cwd, toolchain)
}

/// Register binfmt interpreters
pub(crate) fn register(engine: &Engine, target: &Target, msg_info: &mut MessageInfo) -> Result<()> {
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
    docker.args(["sh", "-c", cmd]);

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

pub(crate) fn cargo_safe_command(cargo_variant: CargoVariant) -> SafeCommand {
    SafeCommand::new(cargo_variant.to_str())
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
            docker.args(["-e", &key]);
        }
    }
}

pub(crate) fn docker_envvars(
    docker: &mut Command,
    options: &DockerOptions,
    dirs: &Directories,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    for ref var in options
        .config
        .env_passthrough(&options.target)?
        .unwrap_or_default()
    {
        validate_env_var(var)?;

        // Only specifying the environment variable name in the "-e"
        // flag forwards the value from the parent shell
        docker.args(["-e", var]);
    }

    let runner = options.config.runner(&options.target)?;
    let cross_runner = format!("CROSS_RUNNER={}", runner.unwrap_or_default());
    docker
        .args(["-e", "PKG_CONFIG_ALLOW_CROSS=1"])
        .args(["-e", &format!("XARGO_HOME={}", dirs.xargo_mount_path())])
        .args(["-e", &format!("CARGO_HOME={}", dirs.cargo_mount_path())])
        .args(["-e", "CARGO_TARGET_DIR=/target"])
        .args(["-e", &cross_runner]);
    if options.cargo_variant.uses_zig() {
        // otherwise, zig has a permission error trying to create the cache
        docker.args(["-e", "XDG_CACHE_HOME=/target/.zig-cache"]);
    }
    add_cargo_configuration_envvars(docker);

    if let Some(username) = id::username().wrap_err("could not get username")? {
        docker.args(["-e", &format!("USER={username}")]);
    }

    if let Ok(value) = env::var("QEMU_STRACE") {
        docker.args(["-e", &format!("QEMU_STRACE={value}")]);
    }

    if let Ok(value) = env::var("CROSS_DEBUG") {
        docker.args(["-e", &format!("CROSS_DEBUG={value}")]);
    }

    if let Ok(value) = env::var("CROSS_CONTAINER_OPTS") {
        if env::var("DOCKER_OPTS").is_ok() {
            msg_info.warn("using both `CROSS_CONTAINER_OPTS` and `DOCKER_OPTS`.")?;
        }
        docker.args(&parse_docker_opts(&value)?);
    } else if let Ok(value) = env::var("DOCKER_OPTS") {
        // FIXME: remove this when we deprecate DOCKER_OPTS.
        docker.args(&parse_docker_opts(&value)?);
    };

    let (major, minor, patch) = match options.rustc_version.as_ref() {
        Some(version) => (version.major, version.minor, version.patch),
        // no toolchain version available, always provide the oldest
        // compiler available. this isn't a major issue because
        // linking will libgcc will not include symbols found in
        // the builtins.
        None => (1, 0, 0),
    };
    docker.args(["-e", &format!("CROSS_RUSTC_MAJOR_VERSION={}", major)]);
    docker.args(["-e", &format!("CROSS_RUSTC_MINOR_VERSION={}", minor)]);
    docker.args(["-e", &format!("CROSS_RUSTC_PATCH_VERSION={}", patch)]);

    Ok(())
}

pub(crate) fn build_command(dirs: &Directories, cmd: &SafeCommand) -> String {
    format!(
        "PATH=\"$PATH\":\"{}/bin\" {:?}",
        dirs.sysroot_mount_path(),
        cmd
    )
}

pub(crate) fn docker_cwd(docker: &mut Command, paths: &DockerPaths) -> Result<()> {
    docker.args(["-w", paths.mount_cwd()]);

    Ok(())
}

pub(crate) fn docker_mount(
    docker: &mut Command,
    options: &DockerOptions,
    paths: &DockerPaths,
    mount_cb: impl Fn(&mut Command, &Path, &Path) -> Result<()>,
    mut store_cb: impl FnMut((String, String)),
) -> Result<()> {
    for ref var in options
        .config
        .env_volumes(&options.target)?
        .unwrap_or_default()
    {
        let (var, value) = validate_env_var(var)?;
        let value = match value {
            Some(v) => Ok(v.to_owned()),
            None => env::var(var),
        };

        // NOTE: we use canonical paths on the host, since it's unambiguous.
        // however, for the mounted paths, we use the same path as was
        // provided. this avoids canonicalizing symlinks which then causes
        // the mounted path to differ from the path expected on the host.
        // for example, if `/tmp` is a symlink to `/private/tmp`, canonicalizing
        // it would lead to us mounting `/tmp/process` to `/private/tmp/process`,
        // which would cause any code relying on `/tmp/process` to break.

        if let Ok(val) = value {
            let canonical_path = file::canonicalize(&val)?;
            let host_path = paths.mount_finder.find_path(&canonical_path, true)?;
            let absolute_path = Path::new(&val).as_posix_absolute()?;
            let mount_path = paths
                .mount_finder
                .find_path(Path::new(&absolute_path), true)?;
            mount_cb(docker, host_path.as_ref(), mount_path.as_ref())?;
            docker.args(["-e", &format!("{}={}", var, mount_path)]);
            store_cb((val, mount_path));
        }
    }

    for path in paths.workspace_dependencies() {
        // NOTE: we use canonical paths here since cargo metadata
        // always canonicalizes paths, so these should be relative
        // to the mounted project directory.
        let canonical_path = file::canonicalize(path)?;
        let host_path = paths.mount_finder.find_path(&canonical_path, true)?;
        let absolute_path = Path::new(path).as_posix_absolute()?;
        let mount_path = paths
            .mount_finder
            .find_path(Path::new(&absolute_path), true)?;
        mount_cb(docker, host_path.as_ref(), mount_path.as_ref())?;
        store_cb((path.to_utf8()?.to_owned(), mount_path));
    }

    Ok(())
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
        docker.args(["--user", &format!("{}:{}", user_id(), group_id(),)]);
    }
}

pub(crate) fn docker_userns(docker: &mut Command) {
    let userns = match env::var("CROSS_CONTAINER_USER_NAMESPACE").ok().as_deref() {
        Some("none") => None,
        None | Some("auto") => Some("host".to_owned()),
        Some(ns) => Some(ns.to_owned()),
    };
    if let Some(ns) = userns {
        docker.args(["--userns", &ns]);
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
        let seccomp = if engine_type.is_docker() && cfg!(target_os = "windows") {
            // docker on windows fails due to a bug in reading the profile
            // https://github.com/docker/for-win/issues/12760
            "unconfined".to_owned()
        } else {
            #[allow(unused_mut)] // target_os = "windows"
            let mut path = metadata
                .target_directory
                .join(target.triple())
                .join("seccomp.json");
            if !path.exists() {
                write_file(&path, false)?.write_all(SECCOMP.as_bytes())?;
            }
            let mut path_string = path.to_utf8()?.to_owned();
            #[cfg(target_os = "windows")]
            if matches!(engine_type, EngineType::Podman | EngineType::PodmanRemote) {
                // podman weirdly expects a WSL path here, and fails otherwise
                path_string = path.as_posix_absolute()?;
            }
            path_string
        };

        docker.args(["--security-opt", &format!("seccomp={}", seccomp)]);
    }

    Ok(())
}

/// Simpler version of [get_image]
pub fn get_image_name(config: &Config, target: &Target, uses_zig: bool) -> Result<String> {
    if let Some(image) = config.image(target)? {
        return Ok(image.name);
    }

    let target_name = match uses_zig {
        true => match config.zig_image(target)? {
            Some(image) => return Ok(image.name),
            None => "zig",
        },
        false => target.triple(),
    };
    let compatible = PROVIDED_IMAGES
        .iter()
        .filter(|p| p.name == target_name)
        .collect::<Vec<_>>();

    if compatible.is_empty() {
        eyre::bail!(
            "`cross` does not provide a Docker image for target {target_name}, \
                   specify a custom image in `Cross.toml`."
        );
    }

    let version = if crate::commit_info().is_empty() {
        env!("CARGO_PKG_VERSION")
    } else {
        "main"
    };

    Ok(compatible
        .get(0)
        .expect("should not be empty")
        .image_name(CROSS_IMAGE, version))
}

pub(crate) fn get_image(config: &Config, target: &Target, uses_zig: bool) -> Result<PossibleImage> {
    if let Some(image) = config.image(target)? {
        return Ok(image);
    }

    let target_name = match uses_zig {
        true => match config.zig_image(target)? {
            Some(image) => return Ok(image),
            None => "zig",
        },
        false => target.triple(),
    };
    let compatible = PROVIDED_IMAGES
        .iter()
        .filter(|p| p.name == target_name)
        .collect::<Vec<_>>();

    if compatible.is_empty() {
        eyre::bail!(
            "`cross` does not provide a Docker image for target {target_name}, \
               specify a custom image in `Cross.toml`."
        );
    }

    let version = if crate::commit_info().is_empty() {
        env!("CARGO_PKG_VERSION")
    } else {
        "main"
    };

    let pick = if compatible.len() == 1 {
        // If only one match, use that
        compatible.get(0).expect("should not be empty")
    } else if compatible
        .iter()
        .filter(|provided| provided.sub.is_none())
        .count()
        == 1
    {
        // if multiple matches, but only one is not a sub-target, pick that one
        compatible
            .iter()
            .find(|provided| provided.sub.is_none())
            .expect("should exists at least one non-sub image in list")
    } else {
        // if there's multiple targets and no option can be chosen, bail
        return Err(eyre::eyre!(
            "`cross` provides multiple images for target {target_name}, \
               specify toolchain in `Cross.toml`."
        )
        .with_note(|| {
            format!(
                "candidates: {}",
                compatible
                    .iter()
                    .map(|provided| format!("\"{}\"", provided.image_name(CROSS_IMAGE, version)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }));
    };

    let mut image: PossibleImage = pick.image_name(CROSS_IMAGE, version).into();

    eyre::ensure!(
        !pick.platforms.is_empty(),
        "platforms for provided image `{image}` are not specified, this is a bug in cross"
    );

    image.toolchain = pick.platforms.to_vec();
    Ok(image)
}

fn docker_read_mount_paths(engine: &Engine) -> Result<Vec<MountDetail>> {
    let hostname = env::var("HOSTNAME").wrap_err("HOSTNAME environment variable not found")?;

    let mut docker: Command = {
        let mut command = subcommand(engine, "inspect");
        command.arg(hostname);
        command
    };

    let output = docker.run_and_get_stdout(&mut Verbosity::Quiet.into())?;
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
        .map_or_else(Vec::new, |v| {
            let make_path = |v: &serde_json::Value| {
                PathBuf::from(&v.as_str().expect("docker mount should be defined"))
            };
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

    pub fn create(engine: &Engine) -> Result<MountFinder> {
        Ok(if engine.in_docker {
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

    fn find_path(&self, path: &Path, host: bool) -> Result<String> {
        if cfg!(target_os = "windows") && host {
            // On Windows, we can not mount the directory name directly.
            // Instead, we convert the path to a linux compatible path.
            return path.to_utf8().map(ToOwned::to_owned);
        } else if cfg!(target_os = "windows") {
            path.as_posix_absolute()
        } else {
            self.find_mount_path(path).as_posix_absolute()
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
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id;

    #[cfg(not(target_os = "windows"))]
    use crate::file::PathExt;

    #[test]
    fn test_docker_user_id() {
        let var = "CROSS_ROOTLESS_CONTAINER_ENGINE";
        let old = env::var(var);
        env::remove_var(var);

        let rootful = format!("\"engine\" \"--user\" \"{}:{}\"", id::user(), id::group());
        let rootless = "\"engine\"".to_owned();

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

        let host = "\"engine\" \"--userns\" \"host\"".to_owned();
        let custom = "\"engine\" \"--userns\" \"custom\"".to_owned();
        let none = "\"engine\"".to_owned();

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
        use crate::rustc::{self, VersionMetaExt};
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

        fn create_engine(msg_info: &mut MessageInfo) -> Result<Engine> {
            Engine::from_path(get_container_engine()?, None, Some(false), msg_info)
        }

        fn cargo_metadata(subdir: bool, msg_info: &mut MessageInfo) -> Result<CargoMetadata> {
            let mut metadata = cargo_metadata_with_args(
                Some(Path::new(env!("CARGO_MANIFEST_DIR"))),
                None,
                msg_info,
            )?
            .ok_or_else(|| eyre::eyre!("could not find cross workspace"))?;

            let root = match subdir {
                true => get_cwd()?.join("member"),
                false => get_cwd()?
                    .parent()
                    .expect("current directory should have a parent")
                    .to_path_buf(),
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

        fn get_toolchain() -> Result<QualifiedToolchain> {
            let host_version_meta = rustc::version_meta()?;
            let host = host_version_meta.host();
            let image_platform =
                crate::docker::ImagePlatform::from_const_target(host.triple().into());
            let sysroot = home()?
                .join(".rustup")
                .join("toolchains")
                .join(host.triple());
            Ok(QualifiedToolchain::new(
                "stable",
                &None,
                &image_platform,
                &sysroot,
            ))
        }

        fn get_directories(
            metadata: CargoMetadata,
            mount_finder: &MountFinder,
        ) -> Result<(Directories, CargoMetadata)> {
            let cwd = get_cwd()?;
            let toolchain = get_toolchain()?;
            Directories::assemble(mount_finder, metadata, &cwd, toolchain)
        }

        #[track_caller]
        fn paths_equal(x: &Path, y: &Path) -> Result<()> {
            assert_eq!(x.as_posix_absolute()?, y.as_posix_absolute()?);
            Ok(())
        }

        #[test]
        #[cfg_attr(cross_sandboxed, ignore)]
        fn test_host() -> Result<()> {
            let vars = unset_env();
            let mount_finder = MountFinder::new(vec![]);
            let metadata = cargo_metadata(false, &mut MessageInfo::default())?;
            let (directories, metadata) = get_directories(metadata, &mount_finder)?;
            paths_equal(&directories.cargo, &home()?.join(".cargo"))?;
            paths_equal(&directories.xargo, &home()?.join(".xargo"))?;
            paths_equal(&directories.host_root, &metadata.workspace_root)?;
            assert_eq!(
                &directories.mount_root,
                &metadata.workspace_root.as_posix_absolute()?
            );
            assert_eq!(&directories.mount_cwd, &get_cwd()?.as_posix_absolute()?);

            reset_env(vars);
            Ok(())
        }

        #[test]
        #[cfg_attr(not(target_os = "linux"), ignore)]
        fn test_docker_in_docker() -> Result<()> {
            let vars = unset_env();

            let mut msg_info = MessageInfo::default();
            let engine = create_engine(&mut msg_info);
            let hostname = env::var("HOSTNAME");
            if engine.is_err() || hostname.is_err() {
                eprintln!("could not get container engine or no hostname found");
                reset_env(vars);
                return Ok(());
            }
            let engine = engine.unwrap();
            if !engine.in_docker {
                eprintln!("not in docker");
                reset_env(vars);
                return Ok(());
            }
            let hostname = hostname.unwrap();
            let output = subcommand(&engine, "inspect")
                .arg(hostname)
                .run_and_get_output(&mut msg_info)?;
            if !output.status.success() {
                eprintln!("inspect failed");
                reset_env(vars);
                return Ok(());
            }

            let mount_finder = MountFinder::create(&engine)?;
            let metadata = cargo_metadata(true, &mut msg_info)?;
            let (directories, _) = get_directories(metadata, &mount_finder)?;
            let mount_finder = MountFinder::new(docker_read_mount_paths(&engine)?);
            let mount_path = |p| mount_finder.find_mount_path(p);

            paths_equal(&directories.cargo, &mount_path(home()?.join(".cargo")))?;
            paths_equal(&directories.xargo, &mount_path(home()?.join(".xargo")))?;
            paths_equal(&directories.host_root, &mount_path(get_cwd()?))?;
            assert_eq!(
                &directories.mount_root,
                &mount_path(get_cwd()?).as_posix_absolute()?
            );
            assert_eq!(
                &directories.mount_cwd,
                &mount_path(get_cwd()?).as_posix_absolute()?
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
            );
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
