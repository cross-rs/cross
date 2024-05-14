use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{env, fs, time};

use super::custom::{Dockerfile, PreBuild};
use super::image::PossibleImage;
use super::Image;
use super::PROVIDED_IMAGES;
use super::{engine::*, ProvidedImage};
use crate::cargo::CargoMetadata;
use crate::config::Config;
use crate::errors::*;
use crate::extensions::{CommandExt, SafeCommand};
use crate::file::{self, write_file, PathExt, ToUtf8};
use crate::id;
use crate::rustc::QualifiedToolchain;
use crate::shell::{ColorChoice, MessageInfo, Verbosity};
use crate::{CommandVariant, OutputExt, Target, TargetTriple};

use rustc_version::Version as RustcVersion;

pub use super::custom::CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX;

pub const CROSS_IMAGE: &str = "ghcr.io/cross-rs";
// note: this is the most common base image for our images
pub const UBUNTU_BASE: &str = "ubuntu:20.04";
pub const DEFAULT_IMAGE_VERSION: &str = if crate::commit_info().is_empty() {
    env!("CARGO_PKG_VERSION")
} else {
    "main"
};

#[derive(Debug)]
pub struct DockerOptions {
    pub engine: Engine,
    pub target: Target,
    pub config: Config,
    pub image: Image,
    pub command_variant: CommandVariant,
    // not all toolchains will provide this
    pub rustc_version: Option<RustcVersion>,
    pub interactive: bool,
}

impl DockerOptions {
    pub fn new(
        engine: Engine,
        target: Target,
        config: Config,
        image: Image,
        cargo_variant: CommandVariant,
        rustc_version: Option<RustcVersion>,
        interactive: bool,
    ) -> DockerOptions {
        DockerOptions {
            engine,
            target,
            config,
            image,
            command_variant: cargo_variant,
            rustc_version,
            interactive,
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
        self.config.dockerfile(&self.target).is_some()
            || self.config.pre_build(&self.target).is_some()
    }

    pub(crate) fn custom_image_build(
        &self,
        paths: &DockerPaths,
        msg_info: &mut MessageInfo,
    ) -> Result<String> {
        let mut image = self.image.clone();
        if self.target.triple() == "arm-unknown-linux-gnueabihf" {
            msg_info.note("cannot install armhf system packages via apt for `arm-unknown-linux-gnueabihf`, since they are for ARMv7a targets but this target is ARMv6. installation of all packages for the armhf architecture has been blocked.")?;
        }

        if let Some(path) = self.config.dockerfile(&self.target) {
            let context = self.config.dockerfile_context(&self.target);

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
                        .dockerfile_build_args(&self.target)
                        .unwrap_or_default(),
                    msg_info,
                )
                .wrap_err("when building dockerfile")?;
        }
        let pre_build = self.config.pre_build(&self.target);

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
        msg_info: &mut MessageInfo,
    ) -> Result<Self> {
        let mount_finder = MountFinder::create(engine, msg_info)?;
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
        self.directories.toolchain_directories().get_sysroot()
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
        self.directories.package_directories().mount_cwd()
    }

    pub fn host_root(&self) -> &Path {
        self.directories.package_directories().host_root()
    }
}

#[derive(Debug)]
pub struct ToolchainDirectories {
    cargo: PathBuf,
    xargo: PathBuf,
    nix_store: Option<PathBuf>,
    toolchain: QualifiedToolchain,
    cargo_mount_path: String,
    xargo_mount_path: String,
    sysroot_mount_path: String,
}

impl ToolchainDirectories {
    pub fn assemble(mount_finder: &MountFinder, toolchain: QualifiedToolchain) -> Result<Self> {
        let home_dir =
            home::home_dir().ok_or_else(|| eyre::eyre!("could not find home directory"))?;
        let cargo = home::cargo_home()?;
        let xargo =
            env::var_os("XARGO_HOME").map_or_else(|| home_dir.join(".xargo"), PathBuf::from);
        // NIX_STORE_DIR is an override of NIX_STORE, which is the path in derivations.
        let nix_store = env::var_os("NIX_STORE_DIR")
            .or_else(|| env::var_os("NIX_STORE"))
            .map(PathBuf::from);

        // create the directories we are going to mount before we mount them,
        // otherwise `docker` will create them but they will be owned by `root`
        // cargo builds all intermediate directories, but fails
        // if it has other issues (such as permission errors).
        file::create_dir_all(&cargo)?;
        file::create_dir_all(&xargo)?;
        if let Some(ref nix_store) = nix_store {
            file::create_dir_all(nix_store)?;
        }

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
                let path = file::canonicalize(store)?;
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

        // canonicalize these once to avoid syscalls
        let sysroot_mount_path = toolchain.get_sysroot().as_posix_absolute()?;

        Ok(ToolchainDirectories {
            cargo,
            xargo,
            nix_store,
            toolchain,
            cargo_mount_path,
            xargo_mount_path,
            sysroot_mount_path,
        })
    }

    pub fn unique_toolchain_identifier(&self) -> Result<String> {
        self.toolchain.unique_toolchain_identifier()
    }

    pub fn unique_container_identifier(&self, triple: &TargetTriple) -> Result<String> {
        self.toolchain.unique_container_identifier(triple)
    }

    pub fn toolchain(&self) -> &QualifiedToolchain {
        &self.toolchain
    }

    pub fn get_sysroot(&self) -> &Path {
        self.toolchain.get_sysroot()
    }

    pub fn host_target(&self) -> &TargetTriple {
        &self.toolchain.host().target
    }

    pub fn cargo(&self) -> &Path {
        &self.cargo
    }

    pub fn cargo_host_path(&self) -> Result<&str> {
        self.cargo.to_utf8()
    }

    pub fn cargo_mount_path(&self) -> &str {
        &self.cargo_mount_path
    }

    pub fn xargo(&self) -> &Path {
        &self.xargo
    }

    pub fn xargo_host_path(&self) -> Result<&str> {
        self.xargo.to_utf8()
    }

    pub fn xargo_mount_path(&self) -> &str {
        &self.xargo_mount_path
    }

    pub fn sysroot_mount_path(&self) -> &str {
        &self.sysroot_mount_path
    }

    pub fn nix_store(&self) -> Option<&Path> {
        self.nix_store.as_deref()
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

#[derive(Debug)]
pub struct PackageDirectories {
    target: PathBuf,
    host_root: PathBuf,
    // both mount fields are WSL paths on windows: they already are POSIX paths
    mount_root: String,
    mount_cwd: String,
}

impl PackageDirectories {
    pub fn assemble(
        mount_finder: &MountFinder,
        metadata: CargoMetadata,
        cwd: &Path,
    ) -> Result<(Self, CargoMetadata)> {
        let target = &metadata.target_directory;
        // see ToolchainDirectories::assemble for creating directories
        create_target_dir(target)?;

        // root is either workspace_root, or, if we're outside the workspace root, the current directory
        let host_root = if metadata.workspace_root.starts_with(cwd) {
            cwd
        } else {
            &metadata.workspace_root
        }
        .to_path_buf();

        // on Windows, we can not mount the directory name directly. Instead, we use wslpath to convert the path to a linux compatible path.
        // NOTE: on unix, host root has already found the mount path
        let mount_root = host_root.as_posix_absolute()?;
        let mount_cwd = cwd.as_posix_absolute()?;

        Ok((
            PackageDirectories {
                target: mount_finder.find_mount_path(target),
                host_root,
                mount_root,
                mount_cwd,
            },
            metadata,
        ))
    }

    pub fn target(&self) -> &Path {
        &self.target
    }

    pub fn host_root(&self) -> &Path {
        &self.host_root
    }

    pub fn mount_root(&self) -> &str {
        &self.mount_root
    }

    pub fn mount_cwd(&self) -> &str {
        &self.mount_cwd
    }
}

#[derive(Debug)]
pub struct Directories {
    toolchain: ToolchainDirectories,
    package: PackageDirectories,
}

impl Directories {
    pub fn assemble(
        mount_finder: &MountFinder,
        metadata: CargoMetadata,
        cwd: &Path,
        toolchain: QualifiedToolchain,
    ) -> Result<(Self, CargoMetadata)> {
        let (package, metadata) = PackageDirectories::assemble(mount_finder, metadata, cwd)?;
        let toolchain = ToolchainDirectories::assemble(mount_finder, toolchain)?;

        Ok((Directories { toolchain, package }, metadata))
    }

    pub fn toolchain_directories(&self) -> &ToolchainDirectories {
        &self.toolchain
    }

    pub fn package_directories(&self) -> &PackageDirectories {
        &self.package
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Dead,
    Exited,
    DoesNotExist,
}

impl ContainerState {
    pub fn new(state: &str) -> Result<Self> {
        match state {
            "created" => Ok(ContainerState::Created),
            "running" => Ok(ContainerState::Running),
            "paused" => Ok(ContainerState::Paused),
            "restarting" => Ok(ContainerState::Restarting),
            "dead" => Ok(ContainerState::Dead),
            "exited" => Ok(ContainerState::Exited),
            "" => Ok(ContainerState::DoesNotExist),
            _ => eyre::bail!("unknown container state: got {state}"),
        }
    }

    #[must_use]
    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Exited | Self::DoesNotExist)
    }

    #[must_use]
    pub fn exists(&self) -> bool {
        !matches!(self, Self::DoesNotExist)
    }
}

// the mount directory for the data volume.
pub const MOUNT_PREFIX: &str = "/cross";
// the prefix used when naming volumes
pub const VOLUME_PREFIX: &str = "cross-";
// default timeout to stop a container (in seconds)
pub const DEFAULT_TIMEOUT: u32 = 2;
// instant kill in case of a non-graceful exit
pub const NO_TIMEOUT: u32 = 0;

pub(crate) static mut CHILD_CONTAINER: ChildContainer = ChildContainer::new();

// the lack of [MessageInfo] is because it'd require a mutable reference,
// since we don't need the functionality behind the [MessageInfo], we can just store the basic
// MessageInfo configurations.
pub(crate) struct ChildContainerInfo {
    engine: Engine,
    name: String,
    timeout: u32,
    color_choice: ColorChoice,
    verbosity: Verbosity,
}

// we need to specify drops for the containers, but we
// also need to ensure the drops are called on a
// termination handler. we use an atomic bool to ensure
// that the drop only gets called once, even if we have
// the signal handle invoked multiple times or it fails.
#[allow(missing_debug_implementations)]
pub struct ChildContainer {
    info: Option<ChildContainerInfo>,
    exists: AtomicBool,
}

impl ChildContainer {
    pub const fn new() -> ChildContainer {
        ChildContainer {
            info: None,
            exists: AtomicBool::new(false),
        }
    }

    pub fn create(engine: Engine, name: String) -> Result<()> {
        // SAFETY: guarded by an atomic swap
        unsafe {
            if !CHILD_CONTAINER.exists.swap(true, Ordering::SeqCst) {
                CHILD_CONTAINER.info = Some(ChildContainerInfo {
                    engine,
                    name,
                    timeout: NO_TIMEOUT,
                    color_choice: ColorChoice::Never,
                    verbosity: Verbosity::Quiet,
                });
                Ok(())
            } else {
                eyre::bail!("attempted to create already existing container.");
            }
        }
    }

    // the static functions have been placed by the internal functions to
    // verify the internal functions are wrapped in atomic load/stores.

    pub fn exists(&self) -> bool {
        self.exists.load(Ordering::SeqCst)
    }

    pub fn exists_static() -> bool {
        // SAFETY: an atomic load.
        unsafe { CHILD_CONTAINER.exists() }
    }

    // when the `docker run` command finished.
    // the container has already exited, so no cleanup required.
    pub fn exit(&mut self) {
        self.exists.store(false, Ordering::SeqCst);
    }

    pub fn exit_static() {
        // SAFETY: an atomic store.
        unsafe {
            CHILD_CONTAINER.exit();
        }
    }

    // when the `docker exec` command finished.
    pub fn finish(&mut self, is_tty: bool, msg_info: &mut MessageInfo) {
        // relax the no-timeout and lack of output
        // ensure we have atomic ordering
        if self.exists() {
            let info = self
                .info
                .as_mut()
                .expect("since we're loaded and exist, child should not be terminated");
            if is_tty {
                info.timeout = DEFAULT_TIMEOUT;
            }
            info.color_choice = msg_info.color_choice;
            info.verbosity = msg_info.verbosity;
        }

        self.terminate();
    }

    pub fn finish_static(is_tty: bool, msg_info: &mut MessageInfo) {
        // SAFETY: internally guarded by an atomic load.
        unsafe {
            CHILD_CONTAINER.finish(is_tty, msg_info);
        }
    }

    // terminate the container early. leaves the struct in a valid
    // state, so it's async safe, but so the container will not
    // be stopped again.
    pub fn terminate(&mut self) {
        if self.exists.swap(false, Ordering::SeqCst) {
            let info = self.info.as_mut().expect(
                "since we're loaded and exist, child should not have been terminated already",
            );
            let mut msg_info = MessageInfo::new(info.color_choice, info.verbosity);
            let container = DockerContainer::new(&info.engine, &info.name);
            container.stop(info.timeout, &mut msg_info).ok();
            container.remove(&mut msg_info).ok();

            self.info = None;
        }
    }
}

impl Drop for ChildContainer {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[derive(Debug)]
pub struct ContainerDataVolume<'a, 'b, 'c> {
    pub(crate) engine: &'a Engine,
    pub(crate) container: &'b str,
    pub(crate) toolchain_dirs: &'c ToolchainDirectories,
}

impl<'a, 'b, 'c> ContainerDataVolume<'a, 'b, 'c> {
    pub const fn new(
        engine: &'a Engine,
        container: &'b str,
        toolchain_dirs: &'c ToolchainDirectories,
    ) -> Self {
        Self {
            engine,
            container,
            toolchain_dirs,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VolumeId {
    Keep(String),
    Discard,
}

impl VolumeId {
    pub fn mount(&self, mount_prefix: &str) -> String {
        match self {
            VolumeId::Keep(ref id) => format!("{id}:{mount_prefix}"),
            VolumeId::Discard => mount_prefix.to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct DockerVolume<'a, 'b> {
    pub(crate) engine: &'a Engine,
    pub(crate) name: &'b str,
}

impl<'a, 'b> DockerVolume<'a, 'b> {
    pub const fn new(engine: &'a Engine, name: &'b str) -> Self {
        Self { engine, name }
    }

    #[track_caller]
    pub fn create(&self, msg_info: &mut MessageInfo) -> Result<ExitStatus> {
        self.engine
            .run_and_get_status(&["volume", "create", self.name], msg_info)
    }

    #[track_caller]
    pub fn remove(&self, msg_info: &mut MessageInfo) -> Result<ExitStatus> {
        self.engine
            .run_and_get_status(&["volume", "rm", self.name], msg_info)
    }

    #[track_caller]
    pub fn exists(&self, msg_info: &mut MessageInfo) -> Result<bool> {
        self.engine
            .run_and_get_output(&["volume", "inspect", self.name], msg_info)
            .map(|output| output.status.success())
    }

    #[track_caller]
    pub fn existing(
        engine: &Engine,
        toolchain: &QualifiedToolchain,
        msg_info: &mut MessageInfo,
    ) -> Result<Vec<String>> {
        let list = engine
            .run_and_get_output(
                &[
                    "volume",
                    "list",
                    "--format",
                    "{{.Name}}",
                    "--filter",
                    &format!("name=^{VOLUME_PREFIX}{}", toolchain),
                ],
                msg_info,
            )?
            .stdout()?;

        if list.is_empty() {
            Ok(vec![])
        } else {
            Ok(list.split('\n').map(ToOwned::to_owned).collect())
        }
    }
}

#[derive(Debug)]
pub struct DockerContainer<'a, 'b> {
    pub(crate) engine: &'a Engine,
    pub(crate) name: &'b str,
}

impl<'a, 'b> DockerContainer<'a, 'b> {
    pub const fn new(engine: &'a Engine, name: &'b str) -> Self {
        Self { engine, name }
    }

    pub fn stop(&self, timeout: u32, msg_info: &mut MessageInfo) -> Result<ExitStatus> {
        self.engine.run_and_get_status(
            &["stop", self.name, "--time", &timeout.to_string()],
            msg_info,
        )
    }

    pub fn stop_default(&self, msg_info: &mut MessageInfo) -> Result<ExitStatus> {
        // we want a faster timeout, since this might happen in signal
        // handler. our containers normally clean up pretty fast, it's
        // only without a pseudo-tty that they don't.
        self.stop(DEFAULT_TIMEOUT, msg_info)
    }

    /// if stopping a container succeeds without a timeout, this command
    /// can fail because the container no longer exists. however, if
    /// the container was killed, we need to cleanup the exited container.
    /// just silence any warnings.
    pub fn remove(&self, msg_info: &mut MessageInfo) -> Result<ExitStatus> {
        self.engine
            .run_and_get_output(&["rm", self.name], msg_info)
            .map(|output| output.status)
    }

    pub fn state(&self, msg_info: &mut MessageInfo) -> Result<ContainerState> {
        let stdout = self
            .engine
            .command()
            .args(["ps", "-a"])
            .args(["--filter", &format!("name={}", self.name)])
            .args(["--format", "{{.State}}"])
            .run_and_get_stdout(msg_info)?;
        ContainerState::new(stdout.trim())
    }
}

pub(crate) fn time_to_millis(timestamp: &time::SystemTime) -> Result<u64> {
    Ok(timestamp
        .duration_since(time::SystemTime::UNIX_EPOCH)?
        .as_millis() as u64)
}

pub(crate) fn time_from_millis(millis: u64) -> time::SystemTime {
    time::SystemTime::UNIX_EPOCH + time::Duration::from_millis(millis)
}

pub(crate) fn now_as_millis() -> Result<u64> {
    time_to_millis(&time::SystemTime::now())
}

const CACHEDIR_TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by cross.
# For information about cache directory tags see https://bford.info/cachedir/";

pub fn create_target_dir(path: &Path) -> Result<()> {
    // cargo creates all paths to the target directory, and writes
    // a cache dir tag only if the path doesn't previously exist.
    if !path.exists() {
        file::create_dir_all(path)?;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.join("CACHEDIR.TAG"))?
            .write_all(CACHEDIR_TAG.as_bytes())?;
    }
    Ok(())
}

impl Engine {
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.path);
        if self.needs_remote() {
            // if we're using podman and not podman-remote, need `--remote`.
            command.arg("--remote");
        }
        command
    }

    pub fn subcommand(&self, cmd: &str) -> Command {
        let mut command = self.command();
        command.arg(cmd);
        command
    }

    #[track_caller]
    pub(crate) fn run_and_get_status(
        &self,
        args: &[&str],
        msg_info: &mut MessageInfo,
    ) -> Result<ExitStatus> {
        self.command().args(args).run_and_get_status(msg_info, true)
    }

    #[track_caller]
    pub(crate) fn run_and_get_output(
        &self,
        args: &[&str],
        msg_info: &mut MessageInfo,
    ) -> Result<Output> {
        self.command().args(args).run_and_get_output(msg_info)
    }

    pub fn parse_opts(value: &str) -> Result<Vec<String>> {
        shell_words::split(value)
            .wrap_err_with(|| format!("could not parse docker opts of {}", value))
    }

    /// Register binfmt interpreters
    pub(crate) fn register_binfmt(
        &self,
        target: &Target,
        msg_info: &mut MessageInfo,
    ) -> Result<()> {
        let cmd = if target.is_windows() {
            // https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html
            "mount binfmt_misc -t binfmt_misc /proc/sys/fs/binfmt_misc && \
                echo ':wine:M::MZ::/usr/bin/run-detectors:' > /proc/sys/fs/binfmt_misc/register"
        } else {
            "apt-get update && apt-get install --no-install-recommends --assume-yes \
                binfmt-support qemu-user-static"
        };

        let mut docker = self.subcommand("run");
        docker.add_userns();
        docker.arg("--privileged");
        docker.arg("--rm");
        docker.arg(UBUNTU_BASE);
        docker.args(["sh", "-c", cmd]);

        docker.run(msg_info, false)
    }
}

fn validate_env_var<'a>(
    var: &'a str,
    warned: &mut bool,
    var_type: &'static str,
    var_syntax: &'static str,
    msg_info: &mut MessageInfo,
) -> Result<(&'a str, Option<&'a str>)> {
    let (key, value) = match var.split_once('=') {
        Some((key, value)) => (key, Some(value)),
        _ => (var, None),
    };

    if value.is_none()
        && !*warned
        && !var
            .chars()
            .all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9'))
    {
        msg_info.warn(format_args!(
            "got {var_type} of \"{var}\" which is not a valid environment variable name. the proper syntax is {var_syntax}"
        ))?;
        *warned = true;
    }

    if key == "CROSS_RUNNER" {
        eyre::bail!(
            "CROSS_RUNNER environment variable name is reserved and cannot be pass through"
        );
    }

    Ok((key, value))
}

impl CommandVariant {
    pub(crate) fn safe_command(&self) -> SafeCommand {
        SafeCommand::new(self.to_str())
    }
}

pub(crate) trait DockerCommandExt {
    fn add_configuration_envvars(&mut self);
    fn add_envvars(
        &mut self,
        options: &DockerOptions,
        dirs: &ToolchainDirectories,
        msg_info: &mut MessageInfo,
    ) -> Result<()>;
    fn add_cwd(&mut self, paths: &DockerPaths) -> Result<()>;
    fn add_build_command(&mut self, dirs: &ToolchainDirectories, cmd: &SafeCommand) -> &mut Self;
    fn add_user_id(&mut self, is_rootless: bool);
    fn add_userns(&mut self);
    fn add_seccomp(
        &mut self,
        engine_type: EngineType,
        target: &Target,
        metadata: &CargoMetadata,
    ) -> Result<()>;
    fn add_mounts(
        &mut self,
        options: &DockerOptions,
        paths: &DockerPaths,
        mount_cb: impl Fn(&mut Command, &Path, &Path) -> Result<()>,
        store_cb: impl FnMut((String, String)),
        msg_info: &mut MessageInfo,
    ) -> Result<()>;
}

impl DockerCommandExt for Command {
    fn add_configuration_envvars(&mut self) {
        let other = &[
            "http_proxy",
            "TERM",
            "RUSTDOCFLAGS",
            "RUSTFLAGS",
            "BROWSER",
            "HTTPS_PROXY",
            "HTTP_TIMEOUT",
            "https_proxy",
            "QEMU_STRACE",
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
        let cross_prefix_skip = &[
            "CROSS_RUNNER",
            "CROSS_RUSTC_MAJOR_VERSION",
            "CROSS_RUSTC_MINOR_VERSION",
            "CROSS_RUSTC_PATCH_VERSION",
        ];
        let is_passthrough = |key: &str| -> bool {
            other.contains(&key)
                || key.starts_with("CARGO_") && !cargo_prefix_skip.contains(&key)
                || key.starts_with("CROSS_") && !cross_prefix_skip.contains(&key)
        };

        // also need to accept any additional flags used to configure
        // cargo or cross, but only pass what's actually present.
        for (key, _) in env::vars() {
            if is_passthrough(&key) {
                self.args(["-e", &key]);
            }
        }
    }

    fn add_envvars(
        &mut self,
        options: &DockerOptions,
        dirs: &ToolchainDirectories,
        msg_info: &mut MessageInfo,
    ) -> Result<()> {
        let mut warned = false;
        for ref var in options
            .config
            .env_passthrough(&options.target)
            .unwrap_or_default()
        {
            validate_env_var(
                var,
                &mut warned,
                "environment variable",
                "`passthrough = [\"ENVVAR=value\"]`",
                msg_info,
            )?;

            // Only specifying the environment variable name in the "-e"
            // flag forwards the value from the parent shell
            self.args(["-e", var]);
        }

        let runner = options.config.runner(&options.target);
        let cross_runner = format!("CROSS_RUNNER={}", runner.unwrap_or_default());
        self.args(["-e", &format!("XARGO_HOME={}", dirs.xargo_mount_path())])
            .args(["-e", &format!("CARGO_HOME={}", dirs.cargo_mount_path())])
            .args([
                "-e",
                &format!("CROSS_RUST_SYSROOT={}", dirs.sysroot_mount_path()),
            ])
            .args(["-e", "CARGO_TARGET_DIR=/target"])
            .args(["-e", &cross_runner]);
        if options.command_variant.uses_zig() {
            // otherwise, zig has a permission error trying to create the cache
            self.args(["-e", "XDG_CACHE_HOME=/target/.zig-cache"]);
        }
        self.add_configuration_envvars();

        if let Some(username) = id::username().wrap_err("could not get username")? {
            self.args(["-e", &format!("USER={username}")]);
        }

        if let Ok(value) = env::var("CROSS_CONTAINER_OPTS") {
            if env::var("DOCKER_OPTS").is_ok() {
                msg_info.warn("using both `CROSS_CONTAINER_OPTS` and `DOCKER_OPTS`.")?;
            }
            self.args(&Engine::parse_opts(&value)?);
        } else if let Ok(value) = env::var("DOCKER_OPTS") {
            // FIXME: remove this when we deprecate DOCKER_OPTS.
            self.args(&Engine::parse_opts(&value)?);
        };

        let (major, minor, patch) = match options.rustc_version.as_ref() {
            Some(version) => (version.major, version.minor, version.patch),
            // no toolchain version available, always provide the oldest
            // compiler available. this isn't a major issue because
            // linking with libgcc will not include symbols found in
            // the builtins.
            None => (1, 0, 0),
        };
        self.args(["-e", &format!("CROSS_RUSTC_MAJOR_VERSION={}", major)]);
        self.args(["-e", &format!("CROSS_RUSTC_MINOR_VERSION={}", minor)]);
        self.args(["-e", &format!("CROSS_RUSTC_PATCH_VERSION={}", patch)]);

        Ok(())
    }

    fn add_cwd(&mut self, paths: &DockerPaths) -> Result<()> {
        self.args(["-w", paths.mount_cwd()]);

        Ok(())
    }

    fn add_build_command(&mut self, dirs: &ToolchainDirectories, cmd: &SafeCommand) -> &mut Self {
        let build_command = format!(
            "PATH=\"$PATH\":\"{}/bin\" {:?}",
            dirs.sysroot_mount_path(),
            cmd
        );
        self.args(["sh", "-c", &build_command])
    }

    fn add_user_id(&mut self, is_rootless: bool) {
        // by default, docker runs as root so we need to specify the user
        // so the resulting file permissions are for the current user.
        // since we can have rootless docker, we provide an override.
        if !is_rootless {
            self.args(["--user", &format!("{}:{}", user_id(), group_id(),)]);
        }
    }

    fn add_userns(&mut self) {
        let userns = match env::var("CROSS_CONTAINER_USER_NAMESPACE").ok().as_deref() {
            Some("none") => None,
            None | Some("auto") => Some("host".to_owned()),
            Some(ns) => Some(ns.to_owned()),
        };
        if let Some(ns) = userns {
            self.args(["--userns", &ns]);
        }
    }

    #[allow(unused_mut, clippy::let_and_return)]
    fn add_seccomp(
        &mut self,
        engine_type: EngineType,
        target: &Target,
        metadata: &CargoMetadata,
    ) -> Result<()> {
        // secured profile based off the docker documentation for denied syscalls:
        // https://docs.docker.com/engine/security/seccomp/#significant-syscalls-blocked-by-the-default-profile
        // note that we've allow listed `clone` and `clone3`, which is necessary
        // to fork the process, and which podman allows by default.
        const SECCOMP: &str = include_str!("seccomp.json");

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

            self.args(["--security-opt", &format!("seccomp={}", seccomp)]);
        }

        Ok(())
    }

    fn add_mounts(
        &mut self,
        options: &DockerOptions,
        paths: &DockerPaths,
        mount_cb: impl Fn(&mut Command, &Path, &Path) -> Result<()>,
        mut store_cb: impl FnMut((String, String)),
        msg_info: &mut MessageInfo,
    ) -> Result<()> {
        let mut warned = false;
        for ref var in options
            .config
            .env_volumes(&options.target)
            .unwrap_or_default()
        {
            let (var, value) = validate_env_var(
                var,
                &mut warned,
                "volume",
                "`volumes = [\"ENVVAR=/path/to/directory\"]`",
                msg_info,
            )?;
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
                let mount_path = Path::new(&val).as_posix_absolute()?;
                mount_cb(self, host_path.as_ref(), mount_path.as_ref())?;
                self.args(["-e", &format!("{}={}", var, mount_path)]);
                store_cb((val, mount_path));
            }
        }

        for path in paths.workspace_dependencies() {
            // NOTE: we use canonical paths here since cargo metadata
            // always canonicalizes paths, so these should be relative
            // to the mounted project directory.
            let canonical_path = file::canonicalize(path)?;
            let host_path = paths.mount_finder.find_path(&canonical_path, true)?;
            let mount_path = path.as_posix_absolute()?;
            mount_cb(self, host_path.as_ref(), mount_path.as_ref())?;
            store_cb((path.to_utf8()?.to_owned(), mount_path));
        }

        Ok(())
    }
}

pub(crate) fn user_id() -> String {
    env::var("CROSS_CONTAINER_UID").unwrap_or_else(|_| id::user().to_string())
}

pub(crate) fn group_id() -> String {
    env::var("CROSS_CONTAINER_GID").unwrap_or_else(|_| id::group().to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum GetImageError {
    #[error(
        "`cross` does not provide a Docker image for target {0}, \
    specify a custom image in `Cross.toml`."
    )]
    NoCompatibleImages(String),
    #[error("platforms for provided image `{0}` are not specified, this is a bug in cross")]
    SpecifiedImageNoPlatform(String),
    #[error(transparent)]
    MultipleImages(eyre::Report),
    #[error(transparent)]
    Other(eyre::Report),
}

fn get_target_name(target: &Target, uses_zig: bool) -> &str {
    if uses_zig {
        "zig"
    } else {
        target.triple()
    }
}

fn get_user_image(
    config: &Config,
    target: &Target,
    uses_zig: bool,
) -> Result<Option<PossibleImage>, GetImageError> {
    let mut image = if uses_zig {
        config.zig_image(target)
    } else {
        config.image(target)
    }
    .map_err(GetImageError::Other)?;

    if let Some(image) = &mut image {
        let target_name = get_target_name(target, uses_zig);
        image.reference.ensure_qualified(target_name);
    }

    Ok(image)
}

fn get_provided_images_for_target(
    target_name: &str,
) -> Result<Vec<&'static ProvidedImage>, GetImageError> {
    let compatible = PROVIDED_IMAGES
        .iter()
        .filter(|p| p.name == target_name)
        .collect::<Vec<_>>();

    if compatible.is_empty() {
        return Err(GetImageError::NoCompatibleImages(target_name.to_owned()));
    }

    Ok(compatible)
}

/// Simpler version of [get_image]
pub fn get_image_name(
    config: &Config,
    target: &Target,
    uses_zig: bool,
) -> Result<String, GetImageError> {
    if let Some(image) = get_user_image(config, target, uses_zig)? {
        return Ok(image.reference.get().to_owned());
    }

    let target_name = get_target_name(target, uses_zig);
    let compatible = get_provided_images_for_target(target_name)?;
    Ok(compatible
        .first()
        .expect("should not be empty")
        .default_image_name())
}

pub fn get_image(
    config: &Config,
    target: &Target,
    uses_zig: bool,
) -> Result<PossibleImage, GetImageError> {
    if let Some(image) = get_user_image(config, target, uses_zig)? {
        return Ok(image);
    }

    let target_name = get_target_name(target, uses_zig);
    let compatible = get_provided_images_for_target(target_name)?;
    let pick = if let [first] = compatible[..] {
        // If only one match, use that
        first
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
        return Err(GetImageError::MultipleImages(
            eyre::eyre!(
                "`cross` provides multiple images for target {target_name}, \
               specify toolchain in `Cross.toml`."
            )
            .with_note(|| {
                format!(
                    "candidates: {}",
                    compatible
                        .iter()
                        .map(|provided| format!("\"{}\"", provided.default_image_name()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }),
        ));
    };

    let image_name = pick.default_image_name();
    if pick.platforms.is_empty() {
        return Err(GetImageError::SpecifiedImageNoPlatform(image_name));
    }

    let mut image: PossibleImage = image_name.into();
    image.toolchain = pick.platforms.to_vec();
    Ok(image)
}

fn docker_inspect_self_mountinfo(engine: &Engine, msg_info: &mut MessageInfo) -> Result<String> {
    if cfg!(not(target_os = "linux")) {
        eyre::bail!("/proc/self/mountinfo is unavailable when target_os != linux");
    }

    // The ID for the current Docker container might be in mountinfo,
    // somewhere in a mount root. Full IDs are 64-char hexadecimal
    // strings, so the first matching path segment in a mount root
    // containing /docker/ is likely to be what we're looking for. See:
    // https://www.kernel.org/doc/Documentation/filesystems/proc.txt
    // https://community.toradex.com/t/15240/4
    let mountinfo = file::read("/proc/self/mountinfo")?;
    let container_id = mountinfo
        .lines()
        .filter_map(|s| s.split(' ').nth(3))
        .filter(|s| s.contains("/docker/"))
        .flat_map(|s| s.split('/'))
        .find(|s| s.len() == 64 && s.as_bytes().iter().all(u8::is_ascii_hexdigit))
        .ok_or_else(|| eyre::eyre!("couldn't find container id in mountinfo"))?;

    engine
        .subcommand("inspect")
        .arg(container_id)
        .run_and_get_stdout(msg_info)
}

fn docker_inspect_self(engine: &Engine, msg_info: &mut MessageInfo) -> Result<String> {
    // Try to find the container ID by looking at HOSTNAME, and fallback to
    // parsing `/proc/self/mountinfo` if HOSTNAME is unset or if there's no
    // container that matches it (necessary e.g. when the container uses
    // `--network=host`, which is act's default, see issue #1321).
    // If `docker inspect` fails with unexpected output, skip the fallback
    // and fail instantly.
    if let Ok(hostname) = env::var("HOSTNAME") {
        let mut command = engine.subcommand("inspect");
        command.arg(hostname);
        let out = command.run_and_get_output(msg_info)?;

        if out.status.success() {
            Ok(out.stdout()?)
        } else {
            let val = serde_json::from_slice::<serde_json::Value>(&out.stdout);
            if let Ok(val) = val {
                if let Some(array) = val.as_array() {
                    // `docker inspect` completed but returned an empty array, most
                    // likely indicating that the hostname isn't a valid container ID.
                    if array.is_empty() {
                        msg_info.debug("docker inspect found no containers matching HOSTNAME, retrying using mountinfo")?;
                        return docker_inspect_self_mountinfo(engine, msg_info);
                    }
                }
            }

            let report = command
                .status_result(msg_info, out.status, Some(&out))
                .expect_err("we know the command failed")
                .to_section_report();
            Err(report)
        }
    } else {
        msg_info.debug("HOSTNAME environment variable is unset")?;
        docker_inspect_self_mountinfo(engine, msg_info)
    }
}

fn docker_read_mount_paths(
    engine: &Engine,
    msg_info: &mut MessageInfo,
) -> Result<Vec<MountDetail>> {
    let output = docker_inspect_self(engine, msg_info)?;
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

    if driver_name.to_lowercase().contains("overlay") {
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

    pub fn create(engine: &Engine, msg_info: &mut MessageInfo) -> Result<MountFinder> {
        Ok(if engine.in_docker {
            MountFinder::new(docker_read_mount_paths(engine, msg_info)?)
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

/// Short hash for identifiers with minimal risk of collision.
pub const PATH_HASH_SHORT: usize = 5;

/// Longer hash to minimize risk of random collisions
/// Collision chance is ~10^-6
pub const PATH_HASH_UNIQUE: usize = 10;

fn path_digest(path: &Path) -> Result<const_sha1::Digest> {
    let buffer = const_sha1::ConstBuffer::from_slice(path.to_utf8()?.as_bytes());
    Ok(const_sha1::sha1(&buffer))
}

pub fn path_hash(path: &Path, count: usize) -> Result<String> {
    Ok(path_digest(path)?
        .to_string()
        .get(..count)
        .unwrap_or_else(|| panic!("sha1 is expected to be at least {count} characters long"))
        .to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{config::Environment, id};

    #[cfg(not(target_os = "windows"))]
    use crate::file::PathExt;

    #[test]
    fn test_docker_user_id() {
        let rootful = format!("\"engine\" \"--user\" \"{}:{}\"", id::user(), id::group());
        let rootless = "\"engine\"".to_owned();

        let test = |noroot, expected| {
            let mut cmd = Command::new("engine");
            cmd.add_user_id(noroot);
            assert_eq!(expected, &format!("{cmd:?}"));
        };

        test(false, &rootful);
        test(true, &rootless);
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
            cmd.add_userns();
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

    #[test]
    fn test_tag_only_image() -> Result<()> {
        let target: Target = TargetTriple::X86_64UnknownLinuxGnu.into();
        let test = |map, expected_ver: &str, expected_ver_zig: &str| -> Result<()> {
            let env = Environment::new(Some(map));
            let config = Config::new_with(None, env);
            for (uses_zig, expected_ver) in [(false, expected_ver), (true, expected_ver_zig)] {
                let expected_image_target = if uses_zig {
                    "zig"
                } else {
                    "x86_64-unknown-linux-gnu"
                };
                let expected = format!("ghcr.io/cross-rs/{expected_image_target}{expected_ver}");

                let image = get_image(&config, &target, uses_zig)?;
                assert_eq!(image.reference.get(), expected);
                let image_name = get_image_name(&config, &target, uses_zig)?;
                assert_eq!(image_name, expected);
            }
            Ok(())
        };

        let default_ver = format!(":{DEFAULT_IMAGE_VERSION}");
        let mut map = HashMap::new();
        test(map.clone(), &default_ver, &default_ver)?;

        map.insert("CROSS_TARGET_X86_64_UNKNOWN_LINUX_GNU_IMAGE", "-centos");
        test(map.clone(), &format!("{default_ver}-centos"), &default_ver)?;

        map.insert("CROSS_TARGET_X86_64_UNKNOWN_LINUX_GNU_IMAGE", ":edge");
        test(map.clone(), ":edge", &default_ver)?;

        map.insert(
            "CROSS_TARGET_X86_64_UNKNOWN_LINUX_GNU_ZIG_IMAGE",
            "@sha256:foobar",
        );
        test(map.clone(), ":edge", "@sha256:foobar")?;

        map.remove("CROSS_TARGET_X86_64_UNKNOWN_LINUX_GNU_IMAGE");
        test(map.clone(), &default_ver, "@sha256:foobar")?;

        Ok(())
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
                false,
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
            let toolchain_dirs = directories.toolchain_directories();
            let package_dirs = directories.package_directories();
            paths_equal(toolchain_dirs.cargo(), &home()?.join(".cargo"))?;
            paths_equal(toolchain_dirs.xargo(), &home()?.join(".xargo"))?;
            paths_equal(package_dirs.host_root(), &metadata.workspace_root)?;
            assert_eq!(
                package_dirs.mount_root(),
                &metadata.workspace_root.as_posix_absolute()?
            );
            assert_eq!(package_dirs.mount_cwd(), &get_cwd()?.as_posix_absolute()?);

            reset_env(vars);
            Ok(())
        }

        #[test]
        #[cfg_attr(not(target_os = "linux"), ignore)]
        fn test_docker_in_docker() -> Result<()> {
            let vars = unset_env();

            let mut msg_info = MessageInfo::default();
            let engine = create_engine(&mut msg_info);
            if engine.is_err() {
                eprintln!("could not get container engine");
                reset_env(vars);
                return Ok(());
            }
            let engine = engine.unwrap();
            if !engine.in_docker {
                eprintln!("not in docker");
                reset_env(vars);
                return Ok(());
            }
            let output = docker_inspect_self(&engine, &mut msg_info);
            if output.is_err() {
                eprintln!("inspect failed");
                reset_env(vars);
                return Ok(());
            }

            let mount_finder = MountFinder::create(&engine, &mut msg_info)?;
            let metadata = cargo_metadata(true, &mut msg_info)?;
            let (directories, _) = get_directories(metadata, &mount_finder)?;
            let toolchain_dirs = directories.toolchain_directories();
            let package_dirs = directories.package_directories();
            let mount_finder = MountFinder::new(docker_read_mount_paths(&engine, &mut msg_info)?);
            let mount_path = |p| mount_finder.find_mount_path(p);

            paths_equal(toolchain_dirs.cargo(), &mount_path(home()?.join(".cargo")))?;
            paths_equal(toolchain_dirs.xargo(), &mount_path(home()?.join(".xargo")))?;
            paths_equal(package_dirs.host_root(), &get_cwd()?)?;
            assert_eq!(package_dirs.mount_root(), &get_cwd()?.as_posix_absolute()?);
            assert_eq!(package_dirs.mount_cwd(), &get_cwd()?.as_posix_absolute()?);

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
