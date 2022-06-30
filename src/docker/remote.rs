use std::collections::BTreeMap;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::{env, fs, time};

use eyre::Context;

use super::engine::Engine;
use super::shared::*;
use crate::cargo::CargoMetadata;
use crate::config::{bool_from_envvar, Config};
use crate::errors::Result;
use crate::extensions::CommandExt;
use crate::file::{self, PathExt, ToUtf8};
use crate::rustc::{self, VersionMetaExt};
use crate::rustup;
use crate::shell::{self, MessageInfo, Stream};
use crate::temp;
use crate::{Host, Target};

// the mount directory for the data volume.
pub const MOUNT_PREFIX: &str = "/cross";

struct DeleteVolume<'a>(&'a Engine, &'a VolumeId, MessageInfo);

impl<'a> Drop for DeleteVolume<'a> {
    fn drop(&mut self) {
        if let VolumeId::Discard(id) = self.1 {
            volume_rm(self.0, id, self.2).ok();
        }
    }
}

struct DeleteContainer<'a>(&'a Engine, &'a str, MessageInfo);

impl<'a> Drop for DeleteContainer<'a> {
    fn drop(&mut self) {
        container_stop(self.0, self.1, self.2).ok();
        container_rm(self.0, self.1, self.2).ok();
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

    pub fn is_stopped(&self) -> bool {
        matches!(self, Self::Exited | Self::DoesNotExist)
    }

    pub fn exists(&self) -> bool {
        !matches!(self, Self::DoesNotExist)
    }
}

#[derive(Debug)]
enum VolumeId {
    Keep(String),
    Discard(String),
}

impl VolumeId {
    fn create(
        engine: &Engine,
        toolchain: &str,
        container: &str,
        msg_info: MessageInfo,
    ) -> Result<Self> {
        if volume_exists(engine, toolchain, msg_info)? {
            Ok(Self::Keep(toolchain.to_string()))
        } else {
            Ok(Self::Discard(container.to_string()))
        }
    }
}

impl AsRef<str> for VolumeId {
    fn as_ref(&self) -> &str {
        match self {
            Self::Keep(s) => s,
            Self::Discard(s) => s,
        }
    }
}

fn create_volume_dir(
    engine: &Engine,
    container: &str,
    dir: &Path,
    msg_info: MessageInfo,
) -> Result<ExitStatus> {
    // make our parent directory if needed
    subcommand(engine, "exec")
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
    msg_info: MessageInfo,
) -> Result<ExitStatus> {
    subcommand(engine, "cp")
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
    msg_info: MessageInfo,
) -> Result<bool> {
    Ok(subcommand(engine, "exec")
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
    msg_info: MessageInfo,
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
    msg_info: MessageInfo,
) -> Result<()> {
    // only need to copy the rustlib files for our current target.
    let triple = target.triple();
    let relpath = Path::new("lib").join("rustlib").join(&triple);
    let src = xargo_dir.join(&relpath);
    let dst = mount_prefix.join("xargo").join(&relpath);
    if Path::new(&src).exists() {
        create_volume_dir(engine, container, dst.parent().unwrap(), msg_info)?;
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
    msg_info: MessageInfo,
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
                .to_string();
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

fn warn_symlinks(had_symlinks: bool, msg_info: MessageInfo) -> Result<()> {
    if had_symlinks {
        shell::warn("copied directory contained symlinks. if the volume the link points to was not mounted, the remote build may fail", msg_info)
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
    msg_info: MessageInfo,
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
    msg_info: MessageInfo,
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
    sysroot: &Path,
    triple: &str,
    mount_prefix: &Path,
    skip_exists: bool,
    msg_info: MessageInfo,
) -> Result<()> {
    // copy over the files for a specific triple
    let dst = mount_prefix.join("rust");
    let rustlib = Path::new("lib").join("rustlib");
    let dst_rustlib = dst.join(&rustlib);
    let src_toolchain = sysroot.join(&rustlib).join(triple);
    let dst_toolchain = dst_rustlib.join(triple);

    // skip if the toolchain already exists. for the host toolchain
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
    sysroot: &Path,
    target: &Target,
    mount_prefix: &Path,
    skip_target: bool,
    msg_info: MessageInfo,
) -> Result<()> {
    let target_triple = target.triple();
    let image_triple = Host::X86_64UnknownLinuxGnu.triple();

    copy_volume_container_rust_base(engine, container, sysroot, mount_prefix, msg_info)?;
    copy_volume_container_rust_manifest(engine, container, sysroot, mount_prefix, msg_info)?;
    copy_volume_container_rust_triple(
        engine,
        container,
        sysroot,
        image_triple,
        mount_prefix,
        false,
        msg_info,
    )?;
    if !skip_target && target_triple != image_triple {
        copy_volume_container_rust_triple(
            engine,
            container,
            sysroot,
            target_triple,
            mount_prefix,
            false,
            msg_info,
        )?;
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
        result.insert(relpath.to_string(), modified);
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
        .filter(|(ref k, ref v1)| {
            previous
                .get(&k.to_string())
                .map(|ref v2| v1 != v2)
                .unwrap_or(true)
        })
        .map(|(k, _)| k.as_str())
        .collect();
    let removed: Vec<&str> = previous
        .iter()
        .filter(|(ref k, _)| !current.contains_key(&k.to_string()))
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
    msg_info: MessageInfo,
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
    msg_info: MessageInfo,
) -> Result<ExitStatus> {
    const PATH: &str = "/tmp/remove_list";
    let mut script = vec![];
    if msg_info.verbose() {
        script.push("set -x".to_string());
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
    subcommand(engine, "cp")
        .arg(tempfile.path())
        .arg(format!("{container}:{PATH}"))
        .run_and_get_status(msg_info, true)?;

    subcommand(engine, "exec")
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
    msg_info: MessageInfo,
) -> Result<()> {
    let copy_all = || {
        if copy_cache {
            copy_volume_files(engine, container, src, dst, msg_info)
        } else {
            copy_volume_files_nocache(engine, container, src, dst, true, msg_info)
        }
    };
    match volume {
        VolumeId::Keep(_) => {
            let parent = temp::dir()?;
            fs::create_dir_all(&parent)?;
            let fingerprint = parent.join(container);
            let current = get_project_fingerprint(src, copy_cache)?;
            if fingerprint.exists() {
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
                copy_all()?;
            }
        }
        VolumeId::Discard(_) => {
            copy_all()?;
        }
    }

    Ok(())
}

fn run_and_get_status(engine: &Engine, args: &[&str], msg_info: MessageInfo) -> Result<ExitStatus> {
    command(engine)
        .args(args)
        .run_and_get_status(msg_info, true)
}

pub fn volume_create(engine: &Engine, volume: &str, msg_info: MessageInfo) -> Result<ExitStatus> {
    run_and_get_status(engine, &["volume", "create", volume], msg_info)
}

pub fn volume_rm(engine: &Engine, volume: &str, msg_info: MessageInfo) -> Result<ExitStatus> {
    run_and_get_status(engine, &["volume", "rm", volume], msg_info)
}

pub fn volume_exists(engine: &Engine, volume: &str, msg_info: MessageInfo) -> Result<bool> {
    command(engine)
        .args(&["volume", "inspect", volume])
        .run_and_get_output(msg_info)
        .map(|output| output.status.success())
}

pub fn container_stop(
    engine: &Engine,
    container: &str,
    msg_info: MessageInfo,
) -> Result<ExitStatus> {
    run_and_get_status(engine, &["stop", container], msg_info)
}

pub fn container_rm(engine: &Engine, container: &str, msg_info: MessageInfo) -> Result<ExitStatus> {
    run_and_get_status(engine, &["rm", container], msg_info)
}

pub fn container_state(
    engine: &Engine,
    container: &str,
    msg_info: MessageInfo,
) -> Result<ContainerState> {
    let stdout = command(engine)
        .args(&["ps", "-a"])
        .args(&["--filter", &format!("name={container}")])
        .args(&["--format", "{{.State}}"])
        .run_and_get_stdout(msg_info)?;
    ContainerState::new(stdout.trim())
}

pub fn unique_toolchain_identifier(sysroot: &Path) -> Result<String> {
    // try to get the commit hash for the currently toolchain, if possible
    // if not, get the default rustc and use the path hash for uniqueness
    let commit_hash = if let Some(version) = rustup::rustc_version_string(sysroot)? {
        rustc::hash_from_version_string(&version, 1)
    } else {
        rustc::version_meta()?.commit_hash()
    };

    let toolchain_name = sysroot.file_name().unwrap().to_utf8()?;
    let toolchain_hash = path_hash(sysroot)?;
    Ok(format!(
        "cross-{toolchain_name}-{toolchain_hash}-{commit_hash}"
    ))
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
        .find(|p| p.manifest_path.parent().unwrap() == workspace_root)
        .unwrap_or_else(|| {
            metadata
                .packages
                .get(0)
                .expect("should have at least 1 package")
        });

    let name = &package.name;
    let triple = target.triple();
    let toolchain_id = unique_toolchain_identifier(&dirs.sysroot)?;
    let project_hash = path_hash(&package.manifest_path)?;
    Ok(format!("{toolchain_id}-{triple}-{name}-{project_hash}"))
}

fn mount_path(val: &Path) -> Result<String> {
    let host_path = file::canonicalize(val)?;
    canonicalize_mount_path(&host_path)
}

#[allow(clippy::too_many_arguments)] // TODO: refactor
pub(crate) fn run(
    engine: &Engine,
    target: &Target,
    args: &[String],
    metadata: &CargoMetadata,
    config: &Config,
    uses_xargo: bool,
    sysroot: &Path,
    msg_info: MessageInfo,
    docker_in_docker: bool,
    cwd: &Path,
) -> Result<ExitStatus> {
    let dirs = Directories::create(engine, metadata, cwd, sysroot, docker_in_docker)?;

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
    let toolchain_id = unique_toolchain_identifier(&dirs.sysroot)?;
    let container = unique_container_identifier(target, metadata, &dirs)?;
    let volume = VolumeId::create(engine, &toolchain_id, &container, msg_info)?;
    let state = container_state(engine, &container, msg_info)?;
    if !state.is_stopped() {
        shell::warn("container {container} was running.", msg_info)?;
        container_stop(engine, &container, msg_info)?;
    }
    if state.exists() {
        shell::warn("container {container} was exited.", msg_info)?;
        container_rm(engine, &container, msg_info)?;
    }
    if let VolumeId::Discard(ref id) = volume {
        if volume_exists(engine, id, msg_info)? {
            shell::warn("temporary volume {container} existed.", msg_info)?;
            volume_rm(engine, id, msg_info)?;
        }
    }

    // 2. create our volume to copy all our data over to
    if let VolumeId::Discard(ref id) = volume {
        volume_create(engine, id, msg_info).wrap_err("when creating volume")?;
    }
    let _volume_deletter = DeleteVolume(engine, &volume, msg_info);

    // 3. create our start container command here
    let mut docker = subcommand(engine, "run");
    docker.args(&["--userns", "host"]);
    docker.args(&["--name", &container]);
    docker.args(&["-v", &format!("{}:{mount_prefix}", volume.as_ref())]);
    docker_envvars(&mut docker, config, target, msg_info)?;

    let mut volumes = vec![];
    let mount_volumes = docker_mount(
        &mut docker,
        metadata,
        config,
        target,
        cwd,
        |_, val| mount_path(val),
        |(src, dst)| volumes.push((src, dst)),
    )
    .wrap_err("could not determine mount points")?;

    docker_seccomp(&mut docker, engine.kind, target, metadata)
        .wrap_err("when copying seccomp profile")?;

    // Prevent `bin` from being mounted inside the Docker container.
    docker.args(&["-v", &format!("{mount_prefix}/cargo/bin")]);

    // When running inside NixOS or using Nix packaging we need to add the Nix
    // Store to the running container so it can load the needed binaries.
    if let Some(ref nix_store) = dirs.nix_store {
        let nix_string = nix_store.to_utf8()?;
        volumes.push((nix_string.to_string(), nix_string.to_string()))
    }

    docker.arg("-d");
    if io::Stdin::is_atty() && io::Stdout::is_atty() && io::Stderr::is_atty() {
        docker.arg("-t");
    }

    docker
        .arg(&image_name(config, target)?)
        // ensure the process never exits until we stop it
        .args(&["sh", "-c", "sleep infinity"])
        .run_and_get_status(msg_info, true)?;
    let _container_deletter = DeleteContainer(engine, &container, msg_info);

    // 4. copy all mounted volumes over
    let copy_cache = env::var("CROSS_REMOTE_COPY_CACHE")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or_default();
    let copy = |src, dst: &PathBuf| {
        if copy_cache {
            copy_volume_files(engine, &container, src, dst, msg_info)
        } else {
            copy_volume_files_nocache(engine, &container, src, dst, true, msg_info)
        }
    };
    let mount_prefix_path = mount_prefix.as_ref();
    if let VolumeId::Discard(_) = volume {
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
            &dirs.sysroot,
            target,
            mount_prefix_path,
            false,
            msg_info,
        )
        .wrap_err("when copying rust")?;
    } else {
        // need to copy over the target triple if it hasn't been previously copied
        copy_volume_container_rust_triple(
            engine,
            &container,
            &dirs.sysroot,
            target.triple(),
            mount_prefix_path,
            true,
            msg_info,
        )
        .wrap_err("when copying rust target files")?;
    }
    let mount_root = if mount_volumes {
        // cannot panic: absolute unix path, must have root
        let rel_mount_root = dirs.mount_root.strip_prefix('/').unwrap();
        let mount_root = mount_prefix_path.join(rel_mount_root);
        if !rel_mount_root.is_empty() {
            create_volume_dir(engine, &container, mount_root.parent().unwrap(), msg_info)
                .wrap_err("when creating mount root")?;
        }
        mount_root
    } else {
        mount_prefix_path.join("project")
    };
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

    let mut copied = vec![
        (&dirs.xargo, mount_prefix_path.join("xargo")),
        (&dirs.cargo, mount_prefix_path.join("cargo")),
        (&dirs.sysroot, mount_prefix_path.join("rust")),
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
            copy(&dirs.target, &target_dir)?;
        } else {
            create_volume_dir(engine, &container, &target_dir, msg_info)?;
        }

        copied.push((&dirs.target, target_dir.clone()));
        target_dir
    };
    for (src, dst) in volumes.iter() {
        let src: &Path = src.as_ref();
        if let Some((psrc, pdst)) = copied.iter().find(|(p, _)| src.starts_with(p)) {
            // path has already been copied over
            let relpath = src.strip_prefix(psrc).unwrap();
            to_symlink.push((pdst.join(relpath), dst));
        } else {
            let rel_dst = dst.strip_prefix('/').unwrap();
            let mount_dst = mount_prefix_path.join(rel_dst);
            if !rel_dst.is_empty() {
                create_volume_dir(engine, &container, mount_dst.parent().unwrap(), msg_info)?;
            }
            copy(src, &mount_dst)?;
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
        final_args.push("--target-dir".to_string());
        final_args.push(target_dir_string);
    }
    let mut cmd = cargo_safe_command(uses_xargo);
    cmd.args(final_args);

    // 5. create symlinks for copied data
    let mut symlink = vec!["set -e pipefail".to_string()];
    if msg_info.verbose() {
        symlink.push("set -x".to_string());
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
    subcommand(engine, "exec")
        .arg(&container)
        .args(&["sh", "-c", &symlink.join("\n")])
        .run_and_get_status(msg_info, false)
        .wrap_err("when creating symlinks to provide consistent host/mount paths")?;

    // 6. execute our cargo command inside the container
    let mut docker = subcommand(engine, "exec");
    docker_user_id(&mut docker, engine.kind);
    docker_cwd(&mut docker, metadata, &dirs, cwd, mount_volumes)?;
    docker.arg(&container);
    docker.args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)]);
    let status = docker
        .run_and_get_status(msg_info, false)
        .map_err(Into::into);

    // 7. copy data from our target dir back to host
    // this might not exist if we ran `clean`.
    if container_path_exists(engine, &container, &target_dir, msg_info)? {
        subcommand(engine, "cp")
            .arg("-a")
            .arg(&format!("{container}:{}", target_dir.as_posix()?))
            .arg(&dirs.target.parent().unwrap())
            .run_and_get_status(msg_info, false)
            .map_err::<eyre::ErrReport, _>(Into::into)?;
    }

    status
}
