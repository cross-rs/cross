use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::sync::atomic::Ordering;

use super::shared::*;
use crate::errors::Result;
use crate::extensions::CommandExt;
use crate::file::{PathExt, ToUtf8};
use crate::shell::{MessageInfo, Stream};
use eyre::Context;

// NOTE: host path must be absolute
fn mount(
    docker: &mut Command,
    host_path: &Path,
    absolute_path: &Path,
    prefix: &str,
    selinux: &str,
) -> Result<()> {
    let mount_path = absolute_path.as_posix_absolute()?;
    docker.args([
        "-v",
        &format!("{}:{prefix}{}{selinux}", host_path.to_utf8()?, mount_path),
    ]);
    Ok(())
}

pub(crate) fn run(
    options: DockerOptions,
    paths: DockerPaths,
    args: &[String],
    msg_info: &mut MessageInfo,
) -> Result<Option<ExitStatus>> {
    let engine = &options.engine;
    let toolchain_dirs = paths.directories.toolchain_directories();
    let package_dirs = paths.directories.package_directories();

    let mut cmd = options.command_variant.safe_command();
    cmd.args(args);

    let mut docker = engine.subcommand("run");
    docker.add_userns();

    // Podman on macOS doesn't support selinux labels, see issue #756
    #[cfg(target_os = "macos")]
    let (selinux, selinux_ro) = if engine.kind.is_podman() {
        ("", ":ro")
    } else {
        (":z", ":z,ro")
    };
    #[cfg(not(target_os = "macos"))]
    let (selinux, selinux_ro) = (":z", ":z,ro");

    options
        .image
        .platform
        .specify_platform(&options.engine, &mut docker);
    docker.add_envvars(&options, toolchain_dirs, msg_info)?;

    docker.add_mounts(
        &options,
        &paths,
        |docker, host, absolute| mount(docker, host, absolute, "", selinux),
        |_| {},
        msg_info,
    )?;

    let container_id = toolchain_dirs.unique_container_identifier(options.target.target())?;
    docker.args(["--name", &container_id]);
    docker.arg("--rm");

    docker
        .add_seccomp(engine.kind, &options.target, &paths.metadata)
        .wrap_err("when copying seccomp profile")?;
    docker.add_user_id(engine.is_rootless);

    docker
        .args([
            "-v",
            &format!(
                "{}:{}{selinux}",
                toolchain_dirs.xargo_host_path()?,
                toolchain_dirs.xargo_mount_path()
            ),
        ])
        .args([
            "-v",
            &format!(
                "{}:{}{selinux}",
                toolchain_dirs.cargo_host_path()?,
                toolchain_dirs.cargo_mount_path()
            ),
        ])
        // Prevent `bin` from being mounted inside the Docker container.
        .args(["-v", &format!("{}/bin", toolchain_dirs.cargo_mount_path())]);

    let host_root = paths.mount_finder.find_mount_path(package_dirs.host_root());
    docker.args([
        "-v",
        &format!(
            "{}:{}{selinux}",
            host_root.to_utf8()?,
            package_dirs.mount_root()
        ),
    ]);

    let sysroot = paths
        .mount_finder
        .find_mount_path(toolchain_dirs.get_sysroot());
    docker
        .args([
            "-v",
            &format!(
                "{}:{}{selinux_ro}",
                sysroot.to_utf8()?,
                toolchain_dirs.sysroot_mount_path()
            ),
        ])
        .args([
            "-v",
            &format!("{}:/target{selinux}", package_dirs.target().to_utf8()?),
        ]);
    docker.add_cwd(&paths)?;

    // When running inside NixOS or using Nix packaging we need to add the Nix
    // Store to the running container so it can load the needed binaries.
    if let Some(nix_store) = toolchain_dirs.nix_store() {
        docker.args([
            "-v",
            &format!(
                "{}:{}{selinux}",
                nix_store.to_utf8()?,
                nix_store.as_posix_absolute()?
            ),
        ]);
    }

    if io::Stdin::is_atty() && io::Stdout::is_atty() && io::Stderr::is_atty() {
        docker.arg("-t");
    }

    if options.interactive {
        docker.arg("-i");
    }

    let mut image_name = options.image.name.clone();
    if options.needs_custom_image() {
        image_name = options
            .custom_image_build(&paths, msg_info)
            .wrap_err("when building custom image")?;
    }

    ChildContainer::create(engine.clone(), container_id)?;
    if msg_info.should_fail() {
        return Ok(None);
    }
    let status = docker
        .arg(&image_name)
        .add_build_command(toolchain_dirs, &cmd)
        .run_and_get_status(msg_info, false);

    // `cargo` generally returns 0 or 101 on completion, but isn't guaranteed
    // to. `ExitStatus::code()` may be None if a signal caused the process to
    // terminate or it may be a known interrupt return status (130, 137, 143).
    // simpler: just test if the program termination handler was called.
    // SAFETY: an atomic load.
    let is_terminated = unsafe { crate::errors::TERMINATED.load(Ordering::SeqCst) };
    if !is_terminated {
        ChildContainer::exit_static();
    }

    status.map(Some)
}
