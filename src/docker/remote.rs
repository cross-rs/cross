use std::collections::BTreeMap;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{env, fs, time};

use eyre::Context;

use super::engine::Engine;
use super::shared::*;
use crate::cargo::CargoMetadata;
use crate::config::bool_from_envvar;
use crate::errors::Result;
use crate::extensions::CommandExt;
use crate::file::{self, PathExt, ToUtf8};
use crate::rustc::{self, QualifiedToolchain, VersionMetaExt};
use crate::shell::{ColorChoice, MessageInfo, Stream, Verbosity};
use crate::temp;
use crate::{Target, TargetTriple};

// the mount directory for the data volume.
pub const MOUNT_PREFIX: &str = "/cross";
// default timeout to stop a container (in seconds)
pub const DEFAULT_TIMEOUT: u32 = 2;
// instant kill in case of a non-graceful exit
pub const NO_TIMEOUT: u32 = 0;

// we need to specify drops for the containers, but we
// also need to ensure the drops are called on a
// termination handler. we use an atomic bool to ensure
// that the drop only gets called once, even if we have
// the signal handle invoked multiple times or it fails.
pub(crate) static mut CONTAINER: Option<DeleteContainer> = None;
pub(crate) static mut CONTAINER_EXISTS: AtomicBool = AtomicBool::new(false);

// it's unlikely that we ever need to erase a line in the destructors,
// and it's better than keep global state everywhere, or keeping a ref
// cell which could have already deleted a line
pub(crate) struct DeleteContainer(Engine, String, u32, ColorChoice, Verbosity);

impl Drop for DeleteContainer {
    fn drop(&mut self) {
        // SAFETY: safe, since guarded by a thread-safe atomic swap.
        unsafe {
            if CONTAINER_EXISTS.swap(false, Ordering::SeqCst) {
                let mut msg_info = MessageInfo::new(self.3, self.4);
                container_stop(&self.0, &self.1, self.2, &mut msg_info).ok();
                container_rm(&self.0, &self.1, &mut msg_info).ok();
            }
        }
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

#[derive(Debug, Clone)]
enum VolumeId {
    Keep(String),
    Discard,
}

impl VolumeId {
    fn create(engine: &Engine, toolchain: &str, msg_info: &mut MessageInfo) -> Result<Self> {
        if volume_exists(engine, toolchain, msg_info)? {
            Ok(Self::Keep(toolchain.to_owned()))
        } else {
            Ok(Self::Discard)
        }
    }
}

// prevent further commands from running if we handled
// a signal earlier, and the volume is exited.
// this isn't required, but avoids unnecessary
// commands while the container is cleaning up.
macro_rules! bail_container_exited {
    () => {{
        if !container_exists() {
            eyre::bail!("container already exited due to signal");
        }
    }};
}

pub fn create_container_deleter(engine: Engine, container: String) {
    // SAFETY: safe, since single-threaded execution.
    unsafe {
        CONTAINER_EXISTS.store(true, Ordering::Relaxed);
        CONTAINER = Some(DeleteContainer(
            engine,
            container,
            NO_TIMEOUT,
            ColorChoice::Never,
            Verbosity::Quiet,
        ));
    }
}

pub fn drop_container(is_tty: bool, msg_info: &mut MessageInfo) {
    // SAFETY: safe, since single-threaded execution.
    unsafe {
        // relax the no-timeout and lack of output
        if let Some(container) = &mut CONTAINER {
            if is_tty {
                container.2 = DEFAULT_TIMEOUT;
            }
            container.3 = msg_info.color_choice;
            container.4 = msg_info.verbosity;
        }
        CONTAINER = None;
    }
}

fn container_exists() -> bool {
    // SAFETY: safe, not mutating an atomic bool
    // this can be more relaxed: just used to ensure
    // that we don't make unnecessary calls, which are
    // safe even if executed, after we've signaled a
    // drop to our container.
    unsafe { CONTAINER_EXISTS.load(Ordering::Relaxed) }
}

fn subcommand_or_exit(engine: &Engine, cmd: &str) -> Result<Command> {
    bail_container_exited!();
    Ok(subcommand(engine, cmd))
}

fn create_volume_dir(
    engine: &Engine,
    container: &str,
    dir: &Path,
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    // make our parent directory if needed
    subcommand_or_exit(engine, "exec")?
        .arg(container)
        .args(&["sh", "-c", &format!("mkdir -p '{}'", dir.as_posix()?)])
        .run_and_get_status(msg_info, false)
}

// copy files for a docker volume, for remote host support
fn copy_volume_files(
    engine: &Engine,
    container: &str,
    src: &Path,
    dst: &Path,
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    subcommand_or_exit(engine, "cp")?
        .arg("-a")
        .arg(src.to_utf8()?)
        .arg(format!("{container}:{}", dst.as_posix()?))
        .run_and_get_status(msg_info, false)
}

fn is_cachedir_tag(path: &Path) -> Result<bool> {
    let mut buffer = [b'0'; 43];
    let mut file = fs::OpenOptions::new().read(true).open(path)?;
    file.read_exact(&mut buffer)?;

    Ok(&buffer == b"Signature: 8a477f597d28d172789f06886806bc55")
}

fn is_cachedir(entry: &fs::DirEntry) -> bool {
    // avoid any cached directories when copying
    // see https://bford.info/cachedir/
    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
        let path = entry.path().join("CACHEDIR.TAG");
        path.exists() && is_cachedir_tag(&path).unwrap_or(false)
    } else {
        false
    }
}

fn container_path_exists(
    engine: &Engine,
    container: &str,
    path: &Path,
    msg_info: &mut MessageInfo,
) -> Result<bool> {
    Ok(subcommand_or_exit(engine, "exec")?
        .arg(container)
        .args(&["bash", "-c", &format!("[[ -d '{}' ]]", path.as_posix()?)])
        .run_and_get_status(msg_info, true)?
        .success())
}

// copy files for a docker volume, for remote host support
fn copy_volume_files_nocache(
    engine: &Engine,
    container: &str,
    src: &Path,
    dst: &Path,
    copy_symlinks: bool,
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    // avoid any cached directories when copying
    // see https://bford.info/cachedir/
    // SAFETY: safe, single-threaded execution.
    let tempdir = unsafe { temp::TempDir::new()? };
    let temppath = tempdir.path();
    let had_symlinks = copy_dir(src, temppath, copy_symlinks, 0, |e, _| is_cachedir(e))?;
    warn_symlinks(had_symlinks, msg_info)?;
    copy_volume_files(engine, container, temppath, dst, msg_info)
}

pub fn copy_volume_container_xargo(
    engine: &Engine,
    container: &str,
    xargo_dir: &Path,
    target: &Target,
    mount_prefix: &Path,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    // only need to copy the rustlib files for our current target.
    let triple = target.triple();
    let relpath = Path::new("lib").join("rustlib").join(&triple);
    let src = xargo_dir.join(&relpath);
    let dst = mount_prefix.join("xargo").join(&relpath);
    if Path::new(&src).exists() {
        create_volume_dir(
            engine,
            container,
            dst.parent().expect("destination should have a parent"),
            msg_info,
        )?;
        copy_volume_files(engine, container, &src, &dst, msg_info)?;
    }

    Ok(())
}

pub fn copy_volume_container_cargo(
    engine: &Engine,
    container: &str,
    cargo_dir: &Path,
    mount_prefix: &Path,
    copy_registry: bool,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    let dst = mount_prefix.join("cargo");
    let copy_registry = env::var("CROSS_REMOTE_COPY_REGISTRY")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or(copy_registry);

    if copy_registry {
        copy_volume_files(engine, container, cargo_dir, &dst, msg_info)?;
    } else {
        // can copy a limit subset of files: the rest is present.
        create_volume_dir(engine, container, &dst, msg_info)?;
        for entry in fs::read_dir(cargo_dir)
            .wrap_err_with(|| format!("when reading directory {cargo_dir:?}"))?
        {
            let file = entry?;
            let basename = file
                .file_name()
                .to_utf8()
                .wrap_err_with(|| format!("when reading file {file:?}"))?
                .to_owned();
            if !basename.starts_with('.') && !matches!(basename.as_ref(), "git" | "registry") {
                copy_volume_files(engine, container, &file.path(), &dst, msg_info)?;
            }
        }
    }

    Ok(())
}

// recursively copy a directory into another
fn copy_dir<Skip>(
    src: &Path,
    dst: &Path,
    copy_symlinks: bool,
    depth: u32,
    skip: Skip,
) -> Result<bool>
where
    Skip: Copy + Fn(&fs::DirEntry, u32) -> bool,
{
    let mut had_symlinks = false;

    for entry in fs::read_dir(src).wrap_err_with(|| format!("when reading directory {src:?}"))? {
        let file = entry?;
        if skip(&file, depth) {
            continue;
        }

        let src_path = file.path();
        let dst_path = dst.join(file.file_name());
        if file.file_type()?.is_file() {
            fs::copy(&src_path, &dst_path)
                .wrap_err_with(|| format!("when copying file {src_path:?} -> {dst_path:?}"))?;
        } else if file.file_type()?.is_dir() {
            fs::create_dir(&dst_path).ok();
            had_symlinks = copy_dir(&src_path, &dst_path, copy_symlinks, depth + 1, skip)?;
        } else if copy_symlinks {
            had_symlinks = true;
            let link_dst = fs::read_link(src_path)?;

            #[cfg(target_family = "unix")]
            {
                std::os::unix::fs::symlink(link_dst, dst_path)?;
            }

            #[cfg(target_family = "windows")]
            {
                let link_dst_absolute = if link_dst.is_absolute() {
                    link_dst.clone()
                } else {
                    // we cannot fail even if the linked to path does not exist.
                    src.join(&link_dst)
                };
                if link_dst_absolute.is_dir() {
                    std::os::windows::fs::symlink_dir(link_dst, dst_path)?;
                } else {
                    // symlink_file handles everything that isn't a directory
                    std::os::windows::fs::symlink_file(link_dst, dst_path)?;
                }
            }
        } else {
            had_symlinks = true;
        }
    }

    Ok(had_symlinks)
}

fn warn_symlinks(had_symlinks: bool, msg_info: &mut MessageInfo) -> Result<()> {
    if had_symlinks {
        msg_info.warn("copied directory contained symlinks. if the volume the link points to was not mounted, the remote build may fail")
    } else {
        Ok(())
    }
}

// copy over files needed for all targets in the toolchain that should never change
fn copy_volume_container_rust_base(
    engine: &Engine,
    container: &str,
    sysroot: &Path,
    mount_prefix: &Path,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    // the rust toolchain is quite large, but most of it isn't needed
    // we need the bin, libexec, and etc directories, and part of the lib directory.
    let dst = mount_prefix.join("rust");
    let rustlib = Path::new("lib").join("rustlib");
    create_volume_dir(engine, container, &dst.join(&rustlib), msg_info)?;
    for basename in ["bin", "libexec", "etc"] {
        let file = sysroot.join(basename);
        copy_volume_files(engine, container, &file, &dst, msg_info)?;
    }

    // the lib directories are rather large, so we want only a subset.
    // now, we use a temp directory for everything else in the libdir
    // we can pretty safely assume we don't have symlinks here.

    // first, copy the shared libraries inside lib, all except rustlib.
    // SAFETY: safe, single-threaded execution.
    let tempdir = unsafe { temp::TempDir::new()? };
    let temppath = tempdir.path();
    fs::create_dir_all(&temppath.join(&rustlib))?;
    let mut had_symlinks = copy_dir(
        &sysroot.join("lib"),
        &temppath.join("lib"),
        true,
        0,
        |e, d| d == 0 && e.file_name() == "rustlib",
    )?;

    // next, copy the src/etc directories inside rustlib
    had_symlinks |= copy_dir(
        &sysroot.join(&rustlib),
        &temppath.join(&rustlib),
        true,
        0,
        |e, d| d == 0 && !(e.file_name() == "src" || e.file_name() == "etc"),
    )?;
    copy_volume_files(engine, container, &temppath.join("lib"), &dst, msg_info)?;

    warn_symlinks(had_symlinks, msg_info)
}

fn copy_volume_container_rust_manifest(
    engine: &Engine,
    container: &str,
    sysroot: &Path,
    mount_prefix: &Path,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    // copy over all the manifest files in rustlib
    // these are small text files containing names/paths to toolchains
    let dst = mount_prefix.join("rust");
    let rustlib = Path::new("lib").join("rustlib");

    // SAFETY: safe, single-threaded execution.
    let tempdir = unsafe { temp::TempDir::new()? };
    let temppath = tempdir.path();
    fs::create_dir_all(&temppath.join(&rustlib))?;
    let had_symlinks = copy_dir(
        &sysroot.join(&rustlib),
        &temppath.join(&rustlib),
        true,
        0,
        |e, d| d != 0 || e.file_type().map(|t| !t.is_file()).unwrap_or(true),
    )?;
    copy_volume_files(engine, container, &temppath.join("lib"), &dst, msg_info)?;

    warn_symlinks(had_symlinks, msg_info)
}

// copy over the toolchain for a specific triple
pub fn copy_volume_container_rust_triple(
    engine: &Engine,
    container: &str,
    toolchain: &QualifiedToolchain,
    target_triple: &TargetTriple,
    mount_prefix: &Path,
    skip_exists: bool,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    let sysroot = toolchain.get_sysroot();
    // copy over the files for a specific triple
    let dst = mount_prefix.join("rust");
    let rustlib = Path::new("lib").join("rustlib");
    let dst_rustlib = dst.join(&rustlib);
    let src_toolchain = sysroot.join(&rustlib).join(target_triple.triple());
    let dst_toolchain = dst_rustlib.join(target_triple.triple());

    // skip if the toolchain target component already exists. for the host toolchain
    // or the first run of the target toolchain, we know it doesn't exist.
    let mut skip = false;
    if skip_exists {
        skip = container_path_exists(engine, container, &dst_toolchain, msg_info)?;
    }
    if !skip {
        copy_volume_files(engine, container, &src_toolchain, &dst_rustlib, msg_info)?;
    }
    if !skip && skip_exists {
        // this means we have a persistent data volume and we have a
        // new target, meaning we might have new manifests as well.
        copy_volume_container_rust_manifest(engine, container, sysroot, mount_prefix, msg_info)?;
    }

    Ok(())
}

pub fn copy_volume_container_rust(
    engine: &Engine,
    container: &str,
    toolchain: &QualifiedToolchain,
    target_triple: Option<&TargetTriple>,
    mount_prefix: &Path,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    copy_volume_container_rust_base(
        engine,
        container,
        toolchain.get_sysroot(),
        mount_prefix,
        msg_info,
    )?;
    copy_volume_container_rust_manifest(
        engine,
        container,
        toolchain.get_sysroot(),
        mount_prefix,
        msg_info,
    )?;
    copy_volume_container_rust_triple(
        engine,
        container,
        toolchain,
        &toolchain.host().target,
        mount_prefix,
        false,
        msg_info,
    )?;
    // TODO: impl Eq
    if let Some(target_triple) = target_triple {
        if target_triple.triple() != toolchain.host().target.triple() {
            copy_volume_container_rust_triple(
                engine,
                container,
                toolchain,
                target_triple,
                mount_prefix,
                false,
                msg_info,
            )?;
        }
    }

    Ok(())
}

type FingerprintMap = BTreeMap<String, time::SystemTime>;

fn parse_project_fingerprint(path: &Path) -> Result<FingerprintMap> {
    let epoch = time::SystemTime::UNIX_EPOCH;
    let file = fs::OpenOptions::new().read(true).open(path)?;
    let reader = io::BufReader::new(file);
    let mut result = BTreeMap::new();
    for line in reader.lines() {
        let line = line?;
        let (timestamp, relpath) = line
            .split_once('\t')
            .ok_or_else(|| eyre::eyre!("unable to parse fingerprint line '{line}'"))?;
        let modified = epoch + time::Duration::from_millis(timestamp.parse::<u64>()?);
        result.insert(relpath.to_owned(), modified);
    }

    Ok(result)
}

fn write_project_fingerprint(path: &Path, fingerprint: &FingerprintMap) -> Result<()> {
    let epoch = time::SystemTime::UNIX_EPOCH;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)?;
    for (relpath, modified) in fingerprint {
        let timestamp = modified.duration_since(epoch)?.as_millis() as u64;
        writeln!(file, "{timestamp}\t{relpath}")?;
    }

    Ok(())
}

fn read_dir_fingerprint(
    home: &Path,
    path: &Path,
    map: &mut FingerprintMap,
    copy_cache: bool,
) -> Result<()> {
    let epoch = time::SystemTime::UNIX_EPOCH;
    for entry in fs::read_dir(path)? {
        let file = entry?;
        let file_type = file.file_type()?;
        // only parse known files types: 0 or 1 of these tests can pass.
        if file_type.is_dir() {
            if copy_cache || !is_cachedir(&file) {
                read_dir_fingerprint(home, &path.join(file.file_name()), map, copy_cache)?;
            }
        } else if file_type.is_file() || file_type.is_symlink() {
            // we're mounting to the same location, so this should fine
            // we need to round the modified date to millis.
            let modified = file.metadata()?.modified()?;
            let millis = modified.duration_since(epoch)?.as_millis() as u64;
            let rounded = epoch + time::Duration::from_millis(millis);
            let relpath = file.path().strip_prefix(home)?.as_posix()?;
            map.insert(relpath, rounded);
        }
    }

    Ok(())
}

fn get_project_fingerprint(home: &Path, copy_cache: bool) -> Result<FingerprintMap> {
    let mut result = BTreeMap::new();
    read_dir_fingerprint(home, home, &mut result, copy_cache)?;
    Ok(result)
}

fn get_fingerprint_difference<'a, 'b>(
    previous: &'a FingerprintMap,
    current: &'b FingerprintMap,
) -> (Vec<&'b str>, Vec<&'a str>) {
    // this can be added or updated
    let changed: Vec<&str> = current
        .iter()
        .filter(|(k, v1)| previous.get(*k).map_or(true, |v2| v1 != &v2))
        .map(|(k, _)| k.as_str())
        .collect();
    let removed: Vec<&str> = previous
        .iter()
        .filter(|(k, _)| !current.contains_key(*k))
        .map(|(k, _)| k.as_str())
        .collect();
    (changed, removed)
}

// copy files for a docker volume, for remote host support
// provides a list of files relative to src.
fn copy_volume_file_list(
    engine: &Engine,
    container: &str,
    src: &Path,
    dst: &Path,
    files: &[&str],
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    // SAFETY: safe, single-threaded execution.
    let tempdir = unsafe { temp::TempDir::new()? };
    let temppath = tempdir.path();
    for file in files {
        let src_path = src.join(file);
        let dst_path = temppath.join(file);
        fs::create_dir_all(dst_path.parent().expect("must have parent"))?;
        fs::copy(&src_path, &dst_path)?;
    }
    copy_volume_files(engine, container, temppath, dst, msg_info)
}

// removed files from a docker volume, for remote host support
// provides a list of files relative to src.
fn remove_volume_file_list(
    engine: &Engine,
    container: &str,
    dst: &Path,
    files: &[&str],
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    const PATH: &str = "/tmp/remove_list";
    let mut script = vec![];
    if msg_info.is_verbose() {
        script.push("set -x".to_owned());
    }
    script.push(format!(
        "cat \"{PATH}\" | while read line; do
    rm -f \"${{line}}\"
done

rm \"{PATH}\"
"
    ));

    // SAFETY: safe, single-threaded execution.
    let mut tempfile = unsafe { temp::TempFile::new()? };
    for file in files {
        writeln!(tempfile.file(), "{}", dst.join(file).as_posix()?)?;
    }

    // need to avoid having hundreds of files on the command, so
    // just provide a single file name.
    subcommand_or_exit(engine, "cp")?
        .arg(tempfile.path())
        .arg(format!("{container}:{PATH}"))
        .run_and_get_status(msg_info, true)?;

    subcommand_or_exit(engine, "exec")?
        .arg(container)
        .args(&["sh", "-c", &script.join("\n")])
        .run_and_get_status(msg_info, true)
}

fn copy_volume_container_project(
    engine: &Engine,
    container: &str,
    src: &Path,
    dst: &Path,
    volume: &VolumeId,
    copy_cache: bool,
    msg_info: &mut MessageInfo,
) -> Result<()> {
    let copy_all = |info: &mut MessageInfo| {
        if copy_cache {
            copy_volume_files(engine, container, src, dst, info)
        } else {
            copy_volume_files_nocache(engine, container, src, dst, true, info)
        }
    };
    match volume {
        VolumeId::Keep(_) => {
            let parent = temp::dir()?;
            fs::create_dir_all(&parent)?;
            let fingerprint = parent.join(container);
            let current = get_project_fingerprint(src, copy_cache)?;
            // need to check if the container path exists, otherwise we might
            // have stale data: the persistent volume was deleted & recreated.
            if fingerprint.exists() && container_path_exists(engine, container, dst, msg_info)? {
                let previous = parse_project_fingerprint(&fingerprint)?;
                let (changed, removed) = get_fingerprint_difference(&previous, &current);
                write_project_fingerprint(&fingerprint, &current)?;

                if !changed.is_empty() {
                    copy_volume_file_list(engine, container, src, dst, &changed, msg_info)?;
                }
                if !removed.is_empty() {
                    remove_volume_file_list(engine, container, dst, &removed, msg_info)?;
                }
            } else {
                write_project_fingerprint(&fingerprint, &current)?;
                copy_all(msg_info)?;
            }
        }
        VolumeId::Discard => {
            copy_all(msg_info)?;
        }
    }

    Ok(())
}

fn run_and_get_status(
    engine: &Engine,
    args: &[&str],
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    command(engine)
        .args(args)
        .run_and_get_status(msg_info, true)
}

fn run_and_get_output(
    engine: &Engine,
    args: &[&str],
    msg_info: &mut MessageInfo,
) -> Result<Output> {
    command(engine).args(args).run_and_get_output(msg_info)
}

pub fn volume_create(
    engine: &Engine,
    volume: &str,
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    run_and_get_status(engine, &["volume", "create", volume], msg_info)
}

pub fn volume_rm(engine: &Engine, volume: &str, msg_info: &mut MessageInfo) -> Result<ExitStatus> {
    run_and_get_status(engine, &["volume", "rm", volume], msg_info)
}

pub fn volume_exists(engine: &Engine, volume: &str, msg_info: &mut MessageInfo) -> Result<bool> {
    run_and_get_output(engine, &["volume", "inspect", volume], msg_info)
        .map(|output| output.status.success())
}

pub fn container_stop(
    engine: &Engine,
    container: &str,
    timeout: u32,
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    run_and_get_status(
        engine,
        &["stop", container, "--time", &timeout.to_string()],
        msg_info,
    )
}

pub fn container_stop_default(
    engine: &Engine,
    container: &str,
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    // we want a faster timeout, since this might happen in signal
    // handler. our containers normally clean up pretty fast, it's
    // only without a pseudo-tty that they don't.
    container_stop(engine, container, DEFAULT_TIMEOUT, msg_info)
}

// if stop succeeds without a timeout, this can have a spurious error
// that is, if the container no longer exists. just silence this.
pub fn container_rm(
    engine: &Engine,
    container: &str,
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    run_and_get_output(engine, &["rm", container], msg_info).map(|output| output.status)
}

pub fn container_state(
    engine: &Engine,
    container: &str,
    msg_info: &mut MessageInfo,
) -> Result<ContainerState> {
    let stdout = command(engine)
        .args(&["ps", "-a"])
        .args(&["--filter", &format!("name={container}")])
        .args(&["--format", "{{.State}}"])
        .run_and_get_stdout(msg_info)?;
    ContainerState::new(stdout.trim())
}

impl QualifiedToolchain {
    pub fn unique_toolchain_identifier(&self) -> Result<String> {
        // try to get the commit hash for the currently toolchain, if possible
        // if not, get the default rustc and use the path hash for uniqueness
        let commit_hash = if let Some(version) = self.rustc_version_string()? {
            rustc::hash_from_version_string(&version, 1)
        } else {
            rustc::version_meta()?.commit_hash()
        };

        let toolchain_name = self
            .get_sysroot()
            .file_name()
            .expect("should be able to get toolchain name")
            .to_utf8()?;
        let toolchain_hash = path_hash(self.get_sysroot())?;
        Ok(format!(
            "cross-{toolchain_name}-{toolchain_hash}-{commit_hash}"
        ))
    }
}

// unique identifier for a given project
pub fn unique_container_identifier(
    target: &Target,
    metadata: &CargoMetadata,
    dirs: &Directories,
) -> Result<String> {
    let workspace_root = &metadata.workspace_root;
    let package = metadata
        .packages
        .iter()
        .find(|p| {
            p.manifest_path
                .parent()
                .expect("manifest path should have a parent directory")
                == workspace_root
        })
        .unwrap_or_else(|| {
            metadata
                .packages
                .get(0)
                .expect("should have at least 1 package")
        });

    let name = &package.name;
    let triple = target.triple();
    let toolchain_id = dirs.toolchain.unique_toolchain_identifier()?;
    let project_hash = path_hash(&package.manifest_path)?;
    Ok(format!("{toolchain_id}-{triple}-{name}-{project_hash}"))
}

fn mount_path(val: &Path) -> Result<String> {
    let host_path = file::canonicalize(val)?;
    canonicalize_mount_path(&host_path)
}

pub(crate) fn run(
    options: DockerOptions,
    paths: DockerPaths,
    args: &[String],
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    let engine = &options.engine;
    let target = &options.target;
    let dirs = &paths.directories;

    let mount_prefix = MOUNT_PREFIX;

    // the logic is broken into the following steps
    // 1. get our unique identifiers and cleanup from a previous run.
    // 2. if not using persistent volumes, create a data volume
    // 3. start our container with the mounted data volume and all envvars
    // 4. copy data into the data volume
    //      with persistent data volumes, copy just copy crate data and
    //      if not present, the toolchain for the current target.
    //      otherwise, copy the entire toolchain, cargo, and crate data
    //      if `CROSS_REMOTE_COPY_CACHE`, copy over the target dir as well
    // 5. create symlinks for all mounted data
    //      ensure the paths are the same as local cross
    // 6. execute our cargo command inside the container
    // 7. copy data from target dir back to host
    // 8. stop container and delete data volume
    //
    // we use structs that wrap the resources to ensure they're dropped
    // in the correct order even on error, to ensure safe cleanup

    // 1. get our unique identifiers and cleanup from a previous run.
    // this can happen if we didn't gracefully exit before
    // note that since we use `docker run --rm`, it's very
    // unlikely the container state existed before.
    let toolchain_id = dirs.toolchain.unique_toolchain_identifier()?;
    let container = unique_container_identifier(target, &paths.metadata, dirs)?;
    let volume = VolumeId::create(engine, &toolchain_id, msg_info)?;
    let state = container_state(engine, &container, msg_info)?;
    if !state.is_stopped() {
        msg_info.warn(format_args!("container {container} was running."))?;
        container_stop_default(engine, &container, msg_info)?;
    }
    if state.exists() {
        msg_info.warn(format_args!("container {container} was exited."))?;
        container_rm(engine, &container, msg_info)?;
    }

    // 2. create our volume to copy all our data over to
    // we actually use an anonymous volume, so it's auto-cleaned up,
    // if we're using a discarded volume.

    // 3. create our start container command here
    let mut docker = subcommand(engine, "run");
    docker_userns(&mut docker);
    options
        .image
        .platform
        .specify_platform(&options.engine, &mut docker);
    docker.args(&["--name", &container]);
    docker.arg("--rm");
    let volume_mount = match volume {
        VolumeId::Keep(ref id) => format!("{id}:{mount_prefix}"),
        VolumeId::Discard => mount_prefix.to_owned(),
    };
    docker.args(&["-v", &volume_mount]);

    let mut volumes = vec![];
    docker_mount(
        &mut docker,
        &options,
        &paths,
        |_, val| mount_path(val),
        |(src, dst)| volumes.push((src, dst)),
    )
    .wrap_err("could not determine mount points")?;

    docker_seccomp(&mut docker, engine.kind, target, &paths.metadata)
        .wrap_err("when copying seccomp profile")?;

    // Prevent `bin` from being mounted inside the Docker container.
    docker.args(&["-v", &format!("{mount_prefix}/cargo/bin")]);

    // When running inside NixOS or using Nix packaging we need to add the Nix
    // Store to the running container so it can load the needed binaries.
    if let Some(ref nix_store) = dirs.nix_store {
        let nix_string = nix_store.to_utf8()?;
        volumes.push((nix_string.to_owned(), nix_string.to_owned()));
    }

    docker.arg("-d");
    let is_tty = io::Stdin::is_atty() && io::Stdout::is_atty() && io::Stderr::is_atty();
    if is_tty {
        docker.arg("-t");
    }

    let mut image_name = options.image.name.clone();

    if options.needs_custom_image() {
        image_name = options
            .custom_image_build(&paths, msg_info)
            .wrap_err("when building custom image")?;
    }

    docker.arg(&image_name);

    if !is_tty {
        // ensure the process never exits until we stop it
        // we only need this infinite loop if we don't allocate
        // a TTY. this has a few issues though: now, the
        // container no longer responds to signals, so the
        // container will need to be sig-killed.
        docker.args(&["sh", "-c", "sleep infinity"]);
    }

    // store first, since failing to non-existing container is fine
    create_container_deleter(engine.clone(), container.clone());
    docker.run_and_get_status(msg_info, true)?;

    // 4. copy all mounted volumes over
    let copy_cache = env::var("CROSS_REMOTE_COPY_CACHE")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or_default();
    let copy = |src, dst: &PathBuf, info: &mut MessageInfo| {
        if copy_cache {
            copy_volume_files(engine, &container, src, dst, info)
        } else {
            copy_volume_files_nocache(engine, &container, src, dst, true, info)
        }
    };
    let mount_prefix_path = mount_prefix.as_ref();
    if let VolumeId::Discard = volume {
        copy_volume_container_xargo(
            engine,
            &container,
            &dirs.xargo,
            target,
            mount_prefix_path,
            msg_info,
        )
        .wrap_err("when copying xargo")?;
        copy_volume_container_cargo(
            engine,
            &container,
            &dirs.cargo,
            mount_prefix_path,
            false,
            msg_info,
        )
        .wrap_err("when copying cargo")?;
        copy_volume_container_rust(
            engine,
            &container,
            &dirs.toolchain,
            Some(target.target()),
            mount_prefix_path,
            msg_info,
        )
        .wrap_err("when copying rust")?;
    } else {
        // need to copy over the target triple if it hasn't been previously copied
        copy_volume_container_rust_triple(
            engine,
            &container,
            &dirs.toolchain,
            target.target(),
            mount_prefix_path,
            true,
            msg_info,
        )
        .wrap_err("when copying rust target files")?;
    }
    // cannot panic: absolute unix path, must have root
    let rel_mount_root = dirs
        .mount_root
        .strip_prefix('/')
        .expect("mount root should be absolute");
    let mount_root = mount_prefix_path.join(rel_mount_root);
    if !rel_mount_root.is_empty() {
        create_volume_dir(
            engine,
            &container,
            mount_root
                .parent()
                .expect("mount root should have a parent directory"),
            msg_info,
        )
        .wrap_err("when creating mount root")?;
    }
    copy_volume_container_project(
        engine,
        &container,
        &dirs.host_root,
        &mount_root,
        &volume,
        copy_cache,
        msg_info,
    )
    .wrap_err("when copying project")?;
    let sysroot = dirs.get_sysroot().to_owned();
    let mut copied = vec![
        (&dirs.xargo, mount_prefix_path.join("xargo")),
        (&dirs.cargo, mount_prefix_path.join("cargo")),
        (&sysroot, mount_prefix_path.join("rust")),
        (&dirs.host_root, mount_root.clone()),
    ];
    let mut to_symlink = vec![];
    let target_dir = file::canonicalize(&dirs.target)?;
    let target_dir = if let Ok(relpath) = target_dir.strip_prefix(&dirs.host_root) {
        mount_root.join(relpath)
    } else {
        // outside project, need to copy the target data over
        // only do if we're copying over cached files.
        let target_dir = mount_prefix_path.join("target");
        if copy_cache {
            copy(&dirs.target, &target_dir, msg_info)?;
        } else {
            create_volume_dir(engine, &container, &target_dir, msg_info)?;
        }

        copied.push((&dirs.target, target_dir.clone()));
        target_dir
    };
    for (src, dst) in &volumes {
        let src: &Path = src.as_ref();
        if let Some((psrc, pdst)) = copied.iter().find(|(p, _)| src.starts_with(p)) {
            // path has already been copied over
            let relpath = src
                .strip_prefix(psrc)
                .expect("source should start with prefix");
            to_symlink.push((pdst.join(relpath), dst));
        } else {
            let rel_dst = dst
                .strip_prefix('/')
                .expect("destination should be absolute");
            let mount_dst = mount_prefix_path.join(rel_dst);
            if !rel_dst.is_empty() {
                create_volume_dir(
                    engine,
                    &container,
                    mount_dst
                        .parent()
                        .expect("destination should have a parent directory"),
                    msg_info,
                )?;
            }
            copy(src, &mount_dst, msg_info)?;
        }
    }

    // `clean` doesn't handle symlinks: it will just unlink the target
    // directory, so we should just substitute it our target directory
    // for it. we'll still have the same end behavior
    let mut final_args = vec![];
    let mut iter = args.iter().cloned();
    let mut has_target_dir = false;
    let target_dir_string = target_dir.as_posix()?;
    while let Some(arg) = iter.next() {
        if arg == "--target-dir" {
            has_target_dir = true;
            final_args.push(arg);
            if iter.next().is_some() {
                final_args.push(target_dir_string.clone());
            }
        } else if arg.starts_with("--target-dir=") {
            has_target_dir = true;
            if arg.split_once('=').is_some() {
                final_args.push(format!("--target-dir={target_dir_string}"));
            }
        } else {
            final_args.push(arg);
        }
    }
    if !has_target_dir {
        final_args.push("--target-dir".to_owned());
        final_args.push(target_dir_string);
    }
    let mut cmd = cargo_safe_command(options.uses_xargo);
    cmd.args(final_args);

    // 5. create symlinks for copied data
    let mut symlink = vec!["set -e pipefail".to_owned()];
    if msg_info.is_verbose() {
        symlink.push("set -x".to_owned());
    }
    symlink.push(format!(
        "chown -R {uid}:{gid} {mount_prefix}",
        uid = user_id(),
        gid = group_id(),
    ));
    // need a simple script to add symlinks, but not override existing files.
    symlink.push(format!(
        "prefix=\"{mount_prefix}\"

symlink_recurse() {{
    for f in \"${{1}}\"/*; do
        dst=${{f#\"$prefix\"}}
        if [ -f \"${{dst}}\" ]; then
            echo \"invalid: got unexpected file at ${{dst}}\" 1>&2
            exit 1
        elif [ -d \"${{dst}}\" ]; then
            symlink_recurse \"${{f}}\"
        else
            ln -s \"${{f}}\" \"${{dst}}\"
        fi
    done
}}

symlink_recurse \"${{prefix}}\"
"
    ));
    for (src, dst) in to_symlink {
        symlink.push(format!("ln -s \"{}\" \"{}\"", src.as_posix()?, dst));
    }
    subcommand_or_exit(engine, "exec")?
        .arg(&container)
        .args(&["sh", "-c", &symlink.join("\n")])
        .run_and_get_status(msg_info, false)
        .wrap_err("when creating symlinks to provide consistent host/mount paths")?;

    // 6. execute our cargo command inside the container
    let mut docker = subcommand(engine, "exec");
    docker_user_id(&mut docker, engine.kind);
    docker_envvars(&mut docker, &options.config, target, msg_info)?;
    docker_cwd(&mut docker, &paths)?;
    docker.arg(&container);
    docker.args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)]);
    bail_container_exited!();
    let status = docker
        .run_and_get_status(msg_info, false)
        .map_err(Into::into);

    // 7. copy data from our target dir back to host
    // this might not exist if we ran `clean`.
    let skip_artifacts = env::var("CROSS_REMOTE_SKIP_BUILD_ARTIFACTS")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or_default();
    bail_container_exited!();
    if !skip_artifacts && container_path_exists(engine, &container, &target_dir, msg_info)? {
        subcommand_or_exit(engine, "cp")?
            .arg("-a")
            .arg(&format!("{container}:{}", target_dir.as_posix()?))
            .arg(
                &dirs
                    .target
                    .parent()
                    .expect("target directory should have a parent"),
            )
            .run_and_get_status(msg_info, false)
            .map_err::<eyre::ErrReport, _>(Into::into)?;
    }

    drop_container(is_tty, msg_info);

    status
}
