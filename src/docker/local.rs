use std::io;
use std::process::ExitStatus;

use super::shared::*;
use crate::errors::Result;
use crate::extensions::CommandExt;
use crate::file::{PathExt, ToUtf8};
use crate::shell::{MessageInfo, Stream};
use eyre::Context;

pub(crate) fn run(
    options: DockerOptions,
    paths: DockerPaths,
    args: &[String],
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    let engine = &options.engine;
    let dirs = &paths.directories;

    let mut cmd = cargo_safe_command(options.uses_xargo);
    cmd.args(args);

    let mut docker = subcommand(engine, "run");
    docker_userns(&mut docker);
    docker_envvars(&mut docker, &options.config, &options.target, msg_info)?;

    let mount_volumes = docker_mount(
        &mut docker,
        &options,
        &paths,
        |docker, val| mount(docker, val, ""),
        |_| {},
    )?;

    docker.arg("--rm");

    docker_seccomp(&mut docker, engine.kind, &options.target, &paths.metadata)
        .wrap_err("when copying seccomp profile")?;
    docker_user_id(&mut docker, engine.kind);

    docker
        .args(&["-v", &format!("{}:/xargo:z", dirs.xargo.to_utf8()?)])
        .args(&["-v", &format!("{}:/cargo:z", dirs.cargo.to_utf8()?)])
        // Prevent `bin` from being mounted inside the Docker container.
        .args(&["-v", "/cargo/bin"]);
    if mount_volumes {
        docker.args(&[
            "-v",
            &format!("{}:{}:z", dirs.host_root.to_utf8()?, dirs.mount_root),
        ]);
    } else {
        docker.args(&["-v", &format!("{}:/project:z", dirs.host_root.to_utf8()?)]);
    }
    docker
        .args(&["-v", &format!("{}:/rust:z,ro", dirs.sysroot.to_utf8()?)])
        .args(&["-v", &format!("{}:/target:z", dirs.target.to_utf8()?)]);
    docker_cwd(&mut docker, &paths, mount_volumes)?;

    // When running inside NixOS or using Nix packaging we need to add the Nix
    // Store to the running container so it can load the needed binaries.
    if let Some(ref nix_store) = dirs.nix_store {
        docker.args(&[
            "-v",
            &format!("{}:{}:z", nix_store.to_utf8()?, nix_store.as_posix()?),
        ]);
    }

    if io::Stdin::is_atty() {
        docker.arg("-i");
        if io::Stdout::is_atty() && io::Stderr::is_atty() {
            docker.arg("-t");
        }
    }
    let mut image = options.image_name()?;
    if options.needs_custom_image() {
        image = options
            .custom_image_build(&paths, msg_info)
            .wrap_err("when building custom image")?;
    }

    docker
        .arg(&image)
        .args(&["sh", "-c", &format!("PATH=$PATH:/rust/bin {:?}", cmd)])
        .run_and_get_status(msg_info, false)
        .map_err(Into::into)
}
