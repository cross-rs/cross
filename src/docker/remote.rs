use std::collections::BTreeMap;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::{env, fs, time};

use eyre::Context;

use super::engine::Engine;
use super::shared::*;
use crate::config::bool_from_envvar;
use crate::errors::Result;
use crate::extensions::CommandExt;
use crate::file::{self, PathExt, ToUtf8};
use crate::rustc::{self, QualifiedToolchain, VersionMetaExt};
use crate::shell::{MessageInfo, Stream};
use crate::temp;
use crate::TargetTriple;

// prevent further commands from running if we handled
// a signal earlier, and the volume is exited.
// this isn't required, but avoids unnecessary
// commands while the container is cleaning up.
macro_rules! bail_container_exited {
    () => {{
        if !ChildContainer::exists_static() {
            eyre::bail!("container already exited due to signal");
        }
    }};
}

#[track_caller]
fn subcommand_or_exit(engine: &Engine, cmd: &str) -> Result<Command> {
    bail_container_exited!();
    Ok(engine.subcommand(cmd))
}

pub fn posix_parent(path: &str) -> Option<&str> {
    Path::new(path).parent()?.to_str()
}

impl<'a, 'b, 'c> ContainerDataVolume<'a, 'b, 'c> {
    // NOTE: `reldir` should be a relative POSIX path to the root directory
    // on windows, this should be something like `mnt/c`. that is, all paths
    // inside the container should not have the mount prefix.
    #[track_caller]
    fn create_dir(
        &self,
        reldir: &str,
        mount_prefix: &str,
        msg_info: &mut MessageInfo,
    ) -> Result<ExitStatus> {
        // make our parent directory if needed
        subcommand_or_exit(self.engine, "exec")?
            .arg(self.container)
            .args(["sh", "-c", &format!("mkdir -p '{mount_prefix}/{reldir}'")])
            .run_and_get_status(msg_info, false)
    }

    /// Copy files for a docker volume
    ///
    /// `reldst` has the same caveats as `reldir` in [`Self::create_dir`].
    ///
    /// ## Note
    ///
    /// if copying from a src directory to dst directory with docker, to
    /// copy the contents from `src` into `dst`, `src` must end with `/.`
    #[track_caller]
    fn copy_files(
        &self,
        src: &Path,
        reldst: &str,
        mount_prefix: &str,
        msg_info: &mut MessageInfo,
    ) -> Result<ExitStatus> {
        if let Some((_, rel)) = reldst.rsplit_once('/') {
            if msg_info.cross_debug
                && src.is_dir()
                && !src.to_string_lossy().ends_with("/.")
                && rel
                    == src
                        .file_name()
                        .expect("filename should be defined as we are a directory")
            {
                msg_info.warn(format_args!(
                    "source is pointing to a directory instead of its contents: {} -> {}\nThis might be a bug. {}",
                    src.as_posix_relative()?,
                    reldst,
                    std::panic::Location::caller()
                ))?;
            }
        }
        subcommand_or_exit(self.engine, "cp")?
            .arg("-a")
            .arg(src.to_utf8()?)
            .arg(format!("{}:{mount_prefix}/{reldst}", self.container))
            .run_and_get_status(msg_info, false)
    }

    /// copy files for a docker volume, does not include cache directories
    ///
    /// ## Note
    ///
    /// if copying from a src directory to dst directory with docker, to
    /// copy the contents from `src` into `dst`, `src` must end with `/.`
    #[track_caller]
    fn copy_files_nocache(
        &self,
        src: &Path,
        reldst: &str,
        mount_prefix: &str,
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
        self.copy_files(&temppath.join("."), reldst, mount_prefix, msg_info)
    }

    // copy files for a docker volume, for remote host support
    // provides a list of files relative to src.
    #[track_caller]
    fn copy_file_list(
        &self,
        src: &Path,
        reldst: &str,
        mount_prefix: &str,
        files: &[&str],
        msg_info: &mut MessageInfo,
    ) -> Result<ExitStatus> {
        // SAFETY: safe, single-threaded execution.
        let tempdir = unsafe { temp::TempDir::new()? };
        let temppath = tempdir.path();
        for file in files {
            let src_path = src.join(file);
            let dst_path = temppath.join(file);
            file::create_dir_all(dst_path.parent().expect("must have parent"))?;
            fs::copy(src_path, &dst_path)?;
        }

        self.copy_files(&temppath.join("."), reldst, mount_prefix, msg_info)
    }

    // removed files from a docker volume, for remote host support
    // provides a list of files relative to src.
    #[track_caller]
    fn remove_file_list(
        &self,
        reldst: &str,
        mount_prefix: &str,
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
            writeln!(tempfile.file(), "{mount_prefix}/{reldst}/{file}")?;
        }

        // need to avoid having hundreds of files on the command, so
        // just provide a single file name.
        subcommand_or_exit(self.engine, "cp")?
            .arg(tempfile.path())
            .arg(format!("{}:{PATH}", self.container))
            .run_and_get_status(msg_info, true)?;

        subcommand_or_exit(self.engine, "exec")?
            .arg(self.container)
            .args(["sh", "-c", &script.join("\n")])
            .run_and_get_status(msg_info, true)
    }

    #[track_caller]
    fn container_path_exists(
        &self,
        relpath: &str,
        mount_prefix: &str,
        msg_info: &mut MessageInfo,
    ) -> Result<bool> {
        Ok(subcommand_or_exit(self.engine, "exec")?
            .arg(self.container)
            .args([
                "bash",
                "-c",
                &format!("[[ -d '{mount_prefix}/{relpath}' ]]"),
            ])
            .run_and_get_status(msg_info, true)?
            .success())
    }

    #[track_caller]
    pub fn copy_xargo(&self, mount_prefix: &str, msg_info: &mut MessageInfo) -> Result<()> {
        let dirs = &self.toolchain_dirs;
        let reldst = dirs.xargo_mount_path_relative()?;
        if dirs.xargo().exists() {
            self.create_dir(
                // this always works, even if we have `/xargo`, since
                // this will be an absolute path. passing an empty path
                // to `create_dir` isn't an issue.
                posix_parent(dirs.xargo_mount_path())
                    .expect("destination should have a parent")
                    .strip_prefix('/')
                    .expect("parent directory must be absolute"),
                mount_prefix,
                msg_info,
            )?;
            self.copy_files(&dirs.xargo().join("."), &reldst, mount_prefix, msg_info)?;
        }

        Ok(())
    }

    #[track_caller]
    pub fn copy_cargo(
        &self,
        mount_prefix: &str,
        copy_registry: bool,
        msg_info: &mut MessageInfo,
    ) -> Result<()> {
        let dirs = &self.toolchain_dirs;
        let reldst = dirs.cargo_mount_path_relative()?;
        let copy_registry = env::var("CROSS_REMOTE_COPY_REGISTRY")
            .map(|s| bool_from_envvar(&s))
            .unwrap_or(copy_registry);

        self.create_dir(&reldst, mount_prefix, msg_info)?;
        if copy_registry {
            self.copy_files(&dirs.cargo().join("."), &reldst, mount_prefix, msg_info)?;
        } else {
            // can copy a limit subset of files: the rest is present.
            for entry in fs::read_dir(dirs.cargo())
                .wrap_err_with(|| format!("when reading directory {:?}", dirs.cargo()))?
            {
                let file = entry?;
                let basename = file
                    .file_name()
                    .to_utf8()
                    .wrap_err_with(|| format!("when reading file {file:?}"))?
                    .to_owned();
                if !basename.starts_with('.') && !matches!(basename.as_ref(), "git" | "registry") {
                    self.copy_files(&file.path(), &reldst, mount_prefix, msg_info)?;
                }
            }
        }

        Ok(())
    }

    // copy over files needed for all targets in the toolchain that should never change
    #[track_caller]
    fn copy_rust_base(&self, mount_prefix: &str, msg_info: &mut MessageInfo) -> Result<()> {
        let dirs = &self.toolchain_dirs;

        // the rust toolchain is quite large, but most of it isn't needed
        // we need the bin, libexec, and etc directories, and part of the lib directory.
        let reldst = dirs.sysroot_mount_path_relative()?;
        let rustlib = "lib/rustlib";
        self.create_dir(&format!("{reldst}/{}", rustlib), mount_prefix, msg_info)?;
        for basename in ["bin", "libexec", "etc"] {
            let file = dirs.get_sysroot().join(basename);
            self.copy_files(&file, &reldst, mount_prefix, msg_info)?;
        }

        // the lib directories are rather large, so we want only a subset.
        // now, we use a temp directory for everything else in the libdir
        // we can pretty safely assume we don't have symlinks here.

        // first, copy the shared libraries inside lib, all except rustlib.
        // SAFETY: safe, single-threaded execution.
        let tempdir = unsafe { temp::TempDir::new()? };
        let temppath = tempdir.path();
        file::create_dir_all(temppath.join(rustlib))?;
        let mut had_symlinks = copy_dir(
            &dirs.get_sysroot().join("lib"),
            &temppath.join("lib"),
            true,
            0,
            |e, d| d == 0 && e.file_name() == "rustlib",
        )?;

        // next, copy the src/etc directories inside rustlib
        had_symlinks |= copy_dir(
            &dirs.get_sysroot().join(rustlib),
            &temppath.join(rustlib),
            true,
            0,
            |e, d| d == 0 && !(e.file_name() == "src" || e.file_name() == "etc"),
        )?;
        self.copy_files(&temppath.join("lib"), &reldst, mount_prefix, msg_info)?;

        warn_symlinks(had_symlinks, msg_info)
    }

    #[track_caller]
    fn copy_rust_manifest(&self, mount_prefix: &str, msg_info: &mut MessageInfo) -> Result<()> {
        let dirs = &self.toolchain_dirs;

        // copy over all the manifest files in rustlib
        // these are small text files containing names/paths to toolchains
        let reldst = dirs.sysroot_mount_path_relative()?;
        let rustlib = "lib/rustlib";

        // SAFETY: safe, single-threaded execution.
        let tempdir = unsafe { temp::TempDir::new()? };
        let temppath = tempdir.path();
        file::create_dir_all(temppath.join(rustlib))?;
        let had_symlinks = copy_dir(
            &dirs.get_sysroot().join(rustlib),
            &temppath.join(rustlib),
            true,
            0,
            |e, d| d != 0 || e.file_type().map(|t| !t.is_file()).unwrap_or(true),
        )?;
        self.copy_files(&temppath.join("lib"), &reldst, mount_prefix, msg_info)?;

        warn_symlinks(had_symlinks, msg_info)
    }

    // copy over the toolchain for a specific triple
    #[track_caller]
    fn copy_rust_triple(
        &self,
        target_triple: &TargetTriple,
        mount_prefix: &str,
        skip_exists: bool,
        msg_info: &mut MessageInfo,
    ) -> Result<()> {
        let dirs = &self.toolchain_dirs;

        // copy over the files for a specific triple
        let reldst = &dirs.sysroot_mount_path_relative()?;
        let rustlib = "lib/rustlib";
        let reldst_rustlib = format!("{reldst}/{rustlib}");
        let src_toolchain = dirs
            .get_sysroot()
            .join(Path::new(rustlib))
            .join(target_triple.triple());
        let reldst_toolchain = format!("{reldst_rustlib}/{}", target_triple.triple());

        // skip if the toolchain target component already exists. for the host toolchain
        // or the first run of the target toolchain, we know it doesn't exist.
        let mut skip = false;
        if skip_exists {
            skip = self.container_path_exists(&reldst_toolchain, mount_prefix, msg_info)?;
        }
        if !skip {
            self.copy_files(&src_toolchain, &reldst_rustlib, mount_prefix, msg_info)?;
        }
        if !skip && skip_exists {
            // this means we have a persistent data volume and we have a
            // new target, meaning we might have new manifests as well.
            self.copy_rust_manifest(mount_prefix, msg_info)?;
        }

        Ok(())
    }

    #[track_caller]
    pub fn copy_rust(
        &self,
        target_triple: Option<&TargetTriple>,
        mount_prefix: &str,
        msg_info: &mut MessageInfo,
    ) -> Result<()> {
        let dirs = &self.toolchain_dirs;

        self.copy_rust_base(mount_prefix, msg_info)?;
        self.copy_rust_manifest(mount_prefix, msg_info)?;
        self.copy_rust_triple(dirs.host_target(), mount_prefix, false, msg_info)?;
        if let Some(target_triple) = target_triple {
            if target_triple.triple() != dirs.host_target().triple() {
                self.copy_rust_triple(target_triple, mount_prefix, false, msg_info)?;
            }
        }

        Ok(())
    }

    #[track_caller]
    fn copy_mount(
        &self,
        src: &Path,
        reldst: &str,
        mount_prefix: &str,
        volume: &VolumeId,
        copy_cache: bool,
        msg_info: &mut MessageInfo,
    ) -> Result<()> {
        let copy_all = |info: &mut MessageInfo| {
            if copy_cache {
                self.copy_files(&src.join("."), reldst, mount_prefix, info)
            } else {
                self.copy_files_nocache(&src.join("."), reldst, mount_prefix, true, info)
            }
        };
        match volume {
            VolumeId::Keep(_) => {
                let parent = temp::dir()?;
                file::create_dir_all(&parent)?;

                let toolchain = &self.toolchain_dirs.toolchain();
                let filename = toolchain.unique_mount_identifier(src)?;
                let fingerprint = parent.join(filename);
                let current = Fingerprint::read_dir(src, copy_cache)?;
                // need to check if the container path exists, otherwise we might
                // have stale data: the persistent volume was deleted & recreated.
                if fingerprint.exists()
                    && self.container_path_exists(reldst, mount_prefix, msg_info)?
                {
                    let previous = Fingerprint::read_file(&fingerprint)?;
                    let (to_copy, to_remove) = previous.difference(&current);
                    if !to_copy.is_empty() {
                        self.copy_file_list(src, reldst, mount_prefix, &to_copy, msg_info)?;
                    }
                    if !to_remove.is_empty() {
                        self.remove_file_list(reldst, mount_prefix, &to_remove, msg_info)?;
                    }

                    // write fingerprint afterwards, in case any failure so we
                    // ensure any changes will be made on subsequent runs
                    current.write_file(&fingerprint)?;
                } else {
                    current.write_file(&fingerprint)?;
                    copy_all(msg_info)?;
                }
            }
            VolumeId::Discard => {
                copy_all(msg_info)?;
            }
        }

        Ok(())
    }
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

#[derive(Debug)]
struct Fingerprint {
    map: BTreeMap<String, time::SystemTime>,
}

impl Fingerprint {
    fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    fn read_file(path: &Path) -> Result<Self> {
        let file = fs::OpenOptions::new().read(true).open(path)?;
        let reader = io::BufReader::new(file);
        let mut map = BTreeMap::new();
        for line in reader.lines() {
            let line = line?;
            let (timestamp, relpath) = line
                .split_once('\t')
                .ok_or_else(|| eyre::eyre!("unable to parse fingerprint line '{line}'"))?;
            let modified = time_from_millis(timestamp.parse::<u64>()?);
            map.insert(relpath.to_owned(), modified);
        }

        Ok(Self { map })
    }

    fn write_file(&self, path: &Path) -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path)?;
        for (relpath, modified) in &self.map {
            let timestamp = time_to_millis(modified)?;
            writeln!(file, "{timestamp}\t{relpath}")?;
        }

        Ok(())
    }

    fn _read_dir(&mut self, home: &Path, path: &Path, copy_cache: bool) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let file = entry?;
            let file_type = file.file_type()?;
            // only parse known files types: 0 or 1 of these tests can pass.
            if file_type.is_dir() {
                if copy_cache || !is_cachedir(&file) {
                    self._read_dir(home, &path.join(file.file_name()), copy_cache)?;
                }
            } else if file_type.is_file() || file_type.is_symlink() {
                // we're mounting to the same location, so this should fine
                // we need to round the modified date to millis.
                let modified = file.metadata()?.modified()?;
                let rounded = time_from_millis(time_to_millis(&modified)?);
                let relpath = file.path().strip_prefix(home)?.as_posix_relative()?;
                self.map.insert(relpath, rounded);
            }
        }

        Ok(())
    }

    fn read_dir(home: &Path, copy_cache: bool) -> Result<Fingerprint> {
        let mut result = Fingerprint::new();
        result._read_dir(home, home, copy_cache)?;
        Ok(result)
    }

    // returns to_copy (added + modified) and to_remove (removed).
    fn difference<'a, 'b>(&'a self, current: &'b Fingerprint) -> (Vec<&'b str>, Vec<&'a str>) {
        let to_copy: Vec<&str> = current
            .map
            .iter()
            .filter(|(k, v1)| self.map.get(*k).map_or(true, |v2| v1 != &v2))
            .map(|(k, _)| k.as_str())
            .collect();
        let to_remove: Vec<&str> = self
            .map
            .iter()
            .filter(|(k, _)| !current.map.contains_key(*k))
            .map(|(k, _)| k.as_str())
            .collect();
        (to_copy, to_remove)
    }
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
        let toolchain_hash = path_hash(self.get_sysroot(), PATH_HASH_SHORT)?;
        Ok(format!(
            "{VOLUME_PREFIX}{toolchain_name}-{toolchain_hash}-{commit_hash}"
        ))
    }

    // unique identifier for a given container. allows the ID to
    // be generated outside a rust package and run multiple times.
    pub fn unique_container_identifier(&self, triple: &TargetTriple) -> Result<String> {
        let toolchain_id = self.unique_toolchain_identifier()?;
        let cwd_path = path_hash(&env::current_dir()?, PATH_HASH_SHORT)?;
        let system_time = now_as_millis()?;
        Ok(format!("{toolchain_id}-{triple}-{cwd_path}-{system_time}"))
    }

    // unique identifier for a given mounted volume
    pub fn unique_mount_identifier(&self, path: &Path) -> Result<String> {
        let toolchain_id = self.unique_toolchain_identifier()?;
        let mount_hash = path_hash(path, PATH_HASH_UNIQUE)?;
        Ok(format!("{toolchain_id}-{mount_hash}"))
    }
}

pub(crate) fn run(
    options: DockerOptions,
    paths: DockerPaths,
    args: &[String],
    subcommand: Option<crate::Subcommand>,
    msg_info: &mut MessageInfo,
) -> Result<Option<ExitStatus>> {
    let engine = &options.engine;
    let target = &options.target;
    let toolchain_dirs = paths.directories.toolchain_directories();
    let package_dirs = paths.directories.package_directories();

    let mount_prefix = MOUNT_PREFIX;

    if options.in_docker() {
        msg_info.warn("remote and docker-in-docker are unlikely to work together when using cross. remote cross uses data volumes, so docker-in-docker should not be required.")?;
    }

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
    let toolchain_id = toolchain_dirs.unique_toolchain_identifier()?;
    let container_id = toolchain_dirs.unique_container_identifier(target.target())?;
    let volume = {
        let existing = DockerVolume::existing(engine, toolchain_dirs.toolchain(), msg_info)?;
        if existing.iter().any(|v| v == &toolchain_id) {
            VolumeId::Keep(toolchain_id)
        } else {
            let partial = format!("{VOLUME_PREFIX}{}", toolchain_dirs.toolchain());
            if existing.iter().any(|v| v.starts_with(&partial)) {
                msg_info.warn(format_args!(
                    "a persistent volume does not exists for `{0}`, but there is a volume for a different version.\n > Create a new volume with `cross-util volumes create --toolchain {0}`",
                    toolchain_dirs.toolchain()
                ))?;
            }
            VolumeId::Discard
        }
    };

    let container = DockerContainer::new(engine, &container_id);
    let state = container.state(msg_info)?;
    if !state.is_stopped() {
        msg_info.warn(format_args!("container {container_id} was running."))?;
        container.stop_default(msg_info)?;
    }
    if state.exists() {
        msg_info.warn(format_args!("container {container_id} was exited."))?;
        container.remove(msg_info)?;
    }

    // 2. create our volume to copy all our data over to
    // we actually use an anonymous volume, so it's auto-cleaned up,
    // if we're using a discarded volume.

    // 3. create our start container command here
    let mut docker = engine.subcommand("run");
    docker.add_userns();
    options
        .image
        .platform
        .specify_platform(&options.engine, &mut docker);
    docker.args(["--name", &container_id]);
    docker.arg("--rm");
    docker.args(["-v", &volume.mount(mount_prefix)]);

    let mut volumes = vec![];
    docker
        .add_mounts(
            &options,
            &paths,
            |_, _, _| Ok(()),
            |(src, dst)| volumes.push((src, dst)),
            msg_info,
        )
        .wrap_err("could not determine mount points")?;

    docker
        .add_seccomp(engine.kind, target, &paths.metadata)
        .wrap_err("when copying seccomp profile")?;

    // Prevent `bin` from being mounted inside the Docker container.
    docker.args(["-v", &format!("{mount_prefix}/cargo/bin")]);

    // When running inside NixOS or using Nix packaging we need to add the Nix
    // Store to the running container so it can load the needed binaries.
    if let Some(nix_store) = toolchain_dirs.nix_store() {
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
        docker.args(["sh", "-c", "sleep infinity"]);
    }

    // store first, since failing to non-existing container is fine
    ChildContainer::create(engine.clone(), container_id.clone())?;
    docker.run_and_get_status(msg_info, true)?;

    // 4. copy all mounted volumes over
    let data_volume = ContainerDataVolume::new(engine, &container_id, toolchain_dirs);
    let copy_cache = env::var("CROSS_REMOTE_COPY_CACHE")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or_default();
    let copy = |src, reldst: &str, info: &mut MessageInfo| {
        data_volume.copy_mount(src, reldst, mount_prefix, &volume, copy_cache, info)
    };
    if let VolumeId::Discard = volume {
        data_volume
            .copy_xargo(mount_prefix, msg_info)
            .wrap_err("when copying xargo")?;
        data_volume
            .copy_cargo(mount_prefix, false, msg_info)
            .wrap_err("when copying cargo")?;
        data_volume
            .copy_rust(Some(target.target()), mount_prefix, msg_info)
            .wrap_err("when copying rust")?;
    } else {
        // need to copy over the target triple if it hasn't been previously copied
        data_volume
            .copy_rust_triple(target.target(), mount_prefix, true, msg_info)
            .wrap_err("when copying rust target files")?;
    }
    // cannot panic: absolute unix path, must have root
    let rel_mount_root = package_dirs
        .mount_root()
        .strip_prefix('/')
        .expect("mount root should be absolute");
    if !rel_mount_root.is_empty() {
        data_volume
            .create_dir(
                posix_parent(rel_mount_root).expect("mount root should have a parent directory"),
                mount_prefix,
                msg_info,
            )
            .wrap_err("when creating mount root")?;
    }
    copy(package_dirs.host_root(), rel_mount_root, msg_info).wrap_err("when copying project")?;
    let sysroot = toolchain_dirs.get_sysroot().to_owned();
    let mut copied = vec![
        (
            toolchain_dirs.xargo(),
            toolchain_dirs.xargo_mount_path_relative()?,
        ),
        (
            toolchain_dirs.cargo(),
            toolchain_dirs.cargo_mount_path_relative()?,
        ),
        (&sysroot, toolchain_dirs.sysroot_mount_path_relative()?),
        (package_dirs.host_root(), rel_mount_root.to_owned()),
    ];
    let mut to_symlink = vec![];
    let target_dir = file::canonicalize(package_dirs.target())?;
    let target_dir = if let Ok(relpath) = target_dir.strip_prefix(package_dirs.host_root()) {
        relpath.as_posix_relative()?
    } else {
        // outside project, need to copy the target data over
        // only do if we're copying over cached files.
        let target_dir = "target".to_owned();
        if copy_cache {
            copy(package_dirs.target(), &target_dir, msg_info)?;
        } else {
            data_volume.create_dir(&target_dir, mount_prefix, msg_info)?;
        }

        copied.push((package_dirs.target(), target_dir.clone()));
        target_dir
    };
    for (src, dst) in &volumes {
        let src: &Path = src.as_ref();
        if let Some((psrc, pdst)) = copied.iter().find(|(p, _)| src.starts_with(p)) {
            // path has already been copied over
            let relpath = src
                .strip_prefix(psrc)
                .expect("source should start with prefix")
                .as_posix_relative()?;
            to_symlink.push((format!("{pdst}/{relpath}"), dst));
        } else {
            let reldst = dst
                .strip_prefix('/')
                .expect("destination should be absolute");
            if !reldst.is_empty() {
                data_volume.create_dir(
                    posix_parent(reldst).expect("destination should have a parent directory"),
                    mount_prefix,
                    msg_info,
                )?;
            }
            copy(src, reldst, msg_info)?;
        }
    }

    let mut cmd = options.command_variant.safe_command();

    if msg_info.should_fail() {
        return Ok(None);
    }

    if !options.command_variant.is_shell() {
        // `clean` doesn't handle symlinks: it will just unlink the target
        // directory, so we should just substitute it our target directory
        // for it. we'll still have the same end behavior
        let mut final_args = vec![];
        let mut iter = args.iter().cloned();
        let mut has_target_dir = false;
        while let Some(arg) = iter.next() {
            if arg == "--target-dir" {
                has_target_dir = true;
                final_args.push(arg);
                if iter.next().is_some() {
                    final_args.push(target_dir.clone());
                }
            } else if arg.starts_with("--target-dir=") {
                has_target_dir = true;
                if arg.split_once('=').is_some() {
                    final_args.push(format!("--target-dir={target_dir}"));
                }
            } else {
                final_args.push(arg);
            }
        }
        if !has_target_dir && subcommand.map_or(true, |s| s.needs_target_in_command()) {
            final_args.push("--target-dir".to_owned());
            final_args.push(target_dir.clone());
        }

        cmd.args(final_args);
    } else {
        cmd.args(args);
    }

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
        symlink.push(format!("ln -s \"{src}\" \"{dst}\"",));
    }
    subcommand_or_exit(engine, "exec")?
        .arg(&container_id)
        .args(["sh", "-c", &symlink.join("\n")])
        .run_and_get_status(msg_info, false)
        .wrap_err("when creating symlinks to provide consistent host/mount paths")?;

    // 6. execute our cargo command inside the container
    let mut docker = engine.subcommand("exec");
    docker.add_user_id(engine.is_rootless);
    docker.add_envvars(&options, toolchain_dirs, msg_info)?;
    docker.add_cwd(&paths)?;
    docker.arg(&container_id);
    docker.add_build_command(toolchain_dirs, &cmd);

    if options.interactive {
        docker.arg("-i");
    }

    bail_container_exited!();
    let status = docker.run_and_get_status(msg_info, false);

    // 7. copy data from our target dir back to host
    // this might not exist if we ran `clean`.
    let skip_artifacts = env::var("CROSS_REMOTE_SKIP_BUILD_ARTIFACTS")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or_default();
    bail_container_exited!();
    let mount_target_dir = format!("{}/{}", package_dirs.mount_root(), target_dir);
    if !skip_artifacts
        && data_volume.container_path_exists(&mount_target_dir, mount_prefix, msg_info)?
    {
        subcommand_or_exit(engine, "cp")?
            .arg("-a")
            .arg(&format!("{container_id}:{mount_target_dir}",))
            .arg(
                package_dirs
                    .target()
                    .parent()
                    .expect("target directory should have a parent"),
            )
            .run_and_get_status(msg_info, false)
            .map_err::<eyre::ErrReport, _>(Into::into)?;
    }

    ChildContainer::finish_static(is_tty, msg_info);

    status.map(Some)
}
