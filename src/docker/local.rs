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

    docker_mount(
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
        .args(&["-v", &format!("{}:/xargo:Z", dirs.xargo.to_utf8()?)])
        .args(&["-v", &format!("{}:/cargo:Z", dirs.cargo.to_utf8()?)])
        // Prevent `bin` from being mounted inside the Docker container.
        .args(&["-v", "/cargo/bin"]);
    docker.args(&[
        "-v",
        &format!("{}:{}:Z", dirs.host_root.to_utf8()?, dirs.mount_root),
    ]);
    docker
        .args(&["-v", &format!("{}:/rust:Z,ro", dirs.sysroot.to_utf8()?)])
        .args(&["-v", &format!("{}:/target:Z", dirs.target.to_utf8()?)]);
    docker_cwd(&mut docker, &paths)?;

    // When running inside NixOS or using Nix packaging we need to add the Nix
    // Store to the running container so it can load the needed binaries.
    if let Some(ref nix_store) = dirs.nix_store {
        docker.args(&[
            "-v",
            &format!("{}:{}:Z", nix_store.to_utf8()?, nix_store.as_posix()?),
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
