use std::io;

use clap::{Args, Subcommand};
use cross::shell::{MessageInfo, Stream};
use cross::{docker, CommandExt};

#[derive(Args, Debug)]
pub struct ListVolumes {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl ListVolumes {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        list_volumes(self, &engine)
    }
}

#[derive(Args, Debug)]
pub struct RemoveAllVolumes {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Force removal of volumes.
    #[clap(short, long)]
    pub force: bool,
    /// Remove volumes. Default is a dry run.
    #[clap(short, long)]
    pub execute: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl RemoveAllVolumes {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        remove_all_volumes(self, &engine)
    }
}

#[derive(Args, Debug)]
pub struct PruneVolumes {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Remove volumes. Default is a dry run.
    #[clap(short, long)]
    pub execute: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl PruneVolumes {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        prune_volumes(self, &engine)
    }
}

#[derive(Args, Debug)]
pub struct CreateVolume {
    /// If cross is running inside a container.
    #[clap(short, long)]
    pub docker_in_docker: bool,
    /// If we should copy the cargo registry to the volume.
    #[clap(short, long)]
    pub copy_registry: bool,
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl CreateVolume {
    pub fn run(self, engine: docker::Engine, channel: Option<&str>) -> cross::Result<()> {
        create_persistent_volume(self, &engine, channel)
    }
}

#[derive(Args, Debug)]
pub struct RemoveVolume {
    /// FIXME: remove in 0.3.0, remains since it's a breaking change.
    #[clap(long, hide = true)]
    pub target: Option<String>,
    /// If cross is running inside a container.
    #[clap(short, long)]
    pub docker_in_docker: bool,
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl RemoveVolume {
    pub fn run(self, engine: docker::Engine, channel: Option<&str>) -> cross::Result<()> {
        remove_persistent_volume(self, &engine, channel)
    }
}

#[derive(Subcommand, Debug)]
pub enum Volumes {
    /// List cross data volumes in local storage.
    List(ListVolumes),
    /// Remove cross data volumes in local storage.
    RemoveAll(RemoveAllVolumes),
    /// Prune volumes not used by any container.
    Prune(PruneVolumes),
    /// Create a persistent data volume for the current toolchain.
    Create(CreateVolume),
    /// Remove a persistent data volume for the current toolchain.
    Remove(RemoveVolume),
}

macro_rules! volumes_get_field {
    ($self:ident, $field:ident $(.$cb:ident)?) => {{
        match $self {
            Volumes::List(l) => l.$field$(.$cb())?,
            Volumes::RemoveAll(l) => l.$field$(.$cb())?,
            Volumes::Prune(l) => l.$field$(.$cb())?,
            Volumes::Create(l) => l.$field$(.$cb())?,
            Volumes::Remove(l) => l.$field$(.$cb())?,
        }
    }};
}

impl Volumes {
    pub fn run(self, engine: docker::Engine, toolchain: Option<&str>) -> cross::Result<()> {
        match self {
            Volumes::List(args) => args.run(engine),
            Volumes::RemoveAll(args) => args.run(engine),
            Volumes::Prune(args) => args.run(engine),
            Volumes::Create(args) => args.run(engine, toolchain),
            Volumes::Remove(args) => args.run(engine, toolchain),
        }
    }

    pub fn engine(&self) -> Option<&str> {
        volumes_get_field!(self, engine.as_deref)
    }

    // FIXME: remove this in v0.3.0.
    pub fn docker_in_docker(&self) -> bool {
        match self {
            Volumes::List(_) => false,
            Volumes::RemoveAll(_) => false,
            Volumes::Prune(_) => false,
            Volumes::Create(l) => l.docker_in_docker,
            Volumes::Remove(l) => l.docker_in_docker,
        }
    }

    pub fn verbose(&self) -> bool {
        volumes_get_field!(self, verbose)
    }

    pub fn quiet(&self) -> bool {
        volumes_get_field!(self, quiet)
    }

    pub fn color(&self) -> Option<&str> {
        volumes_get_field!(self, color.as_deref)
    }
}

#[derive(Args, Debug)]
pub struct ListContainers {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl ListContainers {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        list_containers(self, &engine)
    }
}

#[derive(Args, Debug)]
pub struct RemoveAllContainers {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Force removal of containers.
    #[clap(short, long)]
    pub force: bool,
    /// Remove containers. Default is a dry run.
    #[clap(short, long)]
    pub execute: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl RemoveAllContainers {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        remove_all_containers(self, &engine)
    }
}

#[derive(Subcommand, Debug)]
pub enum Containers {
    /// List cross containers in local storage.
    List(ListContainers),
    /// Stop and remove cross containers in local storage.
    RemoveAll(RemoveAllContainers),
}

macro_rules! containers_get_field {
    ($self:ident, $field:ident $(.$cb:ident)?) => {{
        match $self {
            Containers::List(l) => l.$field$(.$cb())?,
            Containers::RemoveAll(l) => l.$field$(.$cb())?,
        }
    }};
}

impl Containers {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        match self {
            Containers::List(args) => args.run(engine),
            Containers::RemoveAll(args) => args.run(engine),
        }
    }

    pub fn engine(&self) -> Option<&str> {
        containers_get_field!(self, engine.as_deref)
    }

    pub fn verbose(&self) -> bool {
        containers_get_field!(self, verbose)
    }

    pub fn quiet(&self) -> bool {
        containers_get_field!(self, quiet)
    }

    pub fn color(&self) -> Option<&str> {
        containers_get_field!(self, color.as_deref)
    }
}

fn get_cross_volumes(
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<Vec<String>> {
    let stdout = docker::subcommand(engine, "volume")
        .arg("list")
        .args(&["--format", "{{.Name}}"])
        // handles simple regex: ^ for start of line.
        .args(&["--filter", "name=^cross-"])
        .run_and_get_stdout(msg_info)?;

    let mut volumes: Vec<String> = stdout.lines().map(|s| s.to_string()).collect();
    volumes.sort();

    Ok(volumes)
}

pub fn list_volumes(
    ListVolumes {
        verbose,
        quiet,
        color,
        ..
    }: ListVolumes,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let mut msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    for line in get_cross_volumes(engine, &mut msg_info)?.iter() {
        msg_info.print(line)?;
    }

    Ok(())
}

pub fn remove_all_volumes(
    RemoveAllVolumes {
        verbose,
        quiet,
        color,
        force,
        execute,
        ..
    }: RemoveAllVolumes,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let mut msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    let volumes = get_cross_volumes(engine, &mut msg_info)?;

    let mut command = docker::subcommand(engine, "volume");
    command.arg("rm");
    if force {
        command.arg("--force");
    }
    command.args(&volumes);
    if volumes.is_empty() {
        Ok(())
    } else if execute {
        command.run(&mut msg_info, false).map_err(Into::into)
    } else {
        msg_info.note("this is a dry run. to remove the volumes, pass the `--execute` flag.")?;
        command.print(&mut msg_info)?;
        Ok(())
    }
}

pub fn prune_volumes(
    PruneVolumes {
        verbose,
        quiet,
        color,
        execute,
        ..
    }: PruneVolumes,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let mut msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    let mut command = docker::subcommand(engine, "volume");
    command.args(&["prune", "--force"]);
    if execute {
        command.run(&mut msg_info, false).map_err(Into::into)
    } else {
        msg_info.note("this is a dry run. to prune the volumes, pass the `--execute` flag.")?;
        command.print(&mut msg_info)?;
        Ok(())
    }
}

pub fn create_persistent_volume(
    CreateVolume {
        copy_registry,
        verbose,
        quiet,
        color,
        ..
    }: CreateVolume,
    engine: &docker::Engine,
    channel: Option<&str>,
) -> cross::Result<()> {
    let mut msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    // we only need a triple that needs docker: the actual target doesn't matter.
    let triple = cross::Host::X86_64UnknownLinuxGnu.triple();
    let (target, metadata, dirs) =
        docker::get_package_info(engine, triple, channel, &mut msg_info)?;
    let container = docker::remote::unique_container_identifier(&target, &metadata, &dirs)?;
    let volume = docker::remote::unique_toolchain_identifier(&dirs.sysroot)?;

    if docker::remote::volume_exists(engine, &volume, &mut msg_info)? {
        eyre::bail!("Error: volume {volume} already exists.");
    }

    docker::subcommand(engine, "volume")
        .args(&["create", &volume])
        .run_and_get_status(&mut msg_info, false)?;

    // stop the container if it's already running
    let state = docker::remote::container_state(engine, &container, &mut msg_info)?;
    if !state.is_stopped() {
        msg_info.warn("container {container} was running.")?;
        docker::remote::container_stop(engine, &container, &mut msg_info)?;
    }
    if state.exists() {
        msg_info.warn("container {container} was exited.")?;
        docker::remote::container_rm(engine, &container, &mut msg_info)?;
    }

    // create a dummy running container to copy data over
    let mount_prefix = docker::remote::MOUNT_PREFIX;
    let mut docker = docker::subcommand(engine, "run");
    docker.args(&["--name", &container]);
    docker.args(&["-v", &format!("{}:{}", volume, mount_prefix)]);
    docker.arg("-d");
    if io::Stdin::is_atty() && io::Stdout::is_atty() && io::Stderr::is_atty() {
        docker.arg("-t");
    }
    docker.arg(docker::UBUNTU_BASE);
    // ensure the process never exits until we stop it
    docker.args(&["sh", "-c", "sleep infinity"]);
    docker.run_and_get_status(&mut msg_info, false)?;

    docker::remote::copy_volume_container_xargo(
        engine,
        &container,
        &dirs.xargo,
        &target,
        mount_prefix.as_ref(),
        &mut msg_info,
    )?;
    docker::remote::copy_volume_container_cargo(
        engine,
        &container,
        &dirs.cargo,
        mount_prefix.as_ref(),
        copy_registry,
        &mut msg_info,
    )?;
    docker::remote::copy_volume_container_rust(
        engine,
        &container,
        &dirs.sysroot,
        &target,
        mount_prefix.as_ref(),
        true,
        &mut msg_info,
    )?;

    docker::remote::container_stop(engine, &container, &mut msg_info)?;
    docker::remote::container_rm(engine, &container, &mut msg_info)?;

    Ok(())
}

pub fn remove_persistent_volume(
    RemoveVolume {
        verbose,
        quiet,
        color,
        ..
    }: RemoveVolume,
    engine: &docker::Engine,
    channel: Option<&str>,
) -> cross::Result<()> {
    // we only need a triple that needs docker: the actual target doesn't matter.
    let mut msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    let triple = cross::Host::X86_64UnknownLinuxGnu.triple();
    let (_, _, dirs) = docker::get_package_info(engine, triple, channel, &mut msg_info)?;
    let volume = docker::remote::unique_toolchain_identifier(&dirs.sysroot)?;

    if !docker::remote::volume_exists(engine, &volume, &mut msg_info)? {
        eyre::bail!("Error: volume {volume} does not exist.");
    }

    docker::remote::volume_rm(engine, &volume, &mut msg_info)?;

    Ok(())
}

fn get_cross_containers(
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<Vec<String>> {
    let stdout = docker::subcommand(engine, "ps")
        .arg("-a")
        .args(&["--format", "{{.Names}}: {{.State}}"])
        // handles simple regex: ^ for start of line.
        .args(&["--filter", "name=^cross-"])
        .run_and_get_stdout(msg_info)?;

    let mut containers: Vec<String> = stdout.lines().map(|s| s.to_string()).collect();
    containers.sort();

    Ok(containers)
}

pub fn list_containers(
    ListContainers {
        verbose,
        quiet,
        color,
        ..
    }: ListContainers,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let mut msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    for line in get_cross_containers(engine, &mut msg_info)?.iter() {
        msg_info.print(line)?;
    }

    Ok(())
}

pub fn remove_all_containers(
    RemoveAllContainers {
        verbose,
        quiet,
        color,
        force,
        execute,
        ..
    }: RemoveAllContainers,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let mut msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    let containers = get_cross_containers(engine, &mut msg_info)?;
    let mut running = vec![];
    let mut stopped = vec![];
    for container in containers.iter() {
        // cannot fail, formatted as {{.Names}}: {{.State}}
        let (name, state) = container.split_once(':').unwrap();
        let name = name.trim();
        let state = docker::remote::ContainerState::new(state.trim())?;
        if state.is_stopped() {
            stopped.push(name);
        } else {
            running.push(name);
        }
    }

    let mut commands = vec![];
    if !running.is_empty() {
        let mut stop = docker::subcommand(engine, "stop");
        stop.args(&running);
        commands.push(stop);
    }

    if !(stopped.is_empty() && running.is_empty()) {
        let mut rm = docker::subcommand(engine, "rm");
        if force {
            rm.arg("--force");
        }
        rm.args(&running);
        rm.args(&stopped);
        commands.push(rm);
    }
    if execute {
        for mut command in commands {
            command.run(&mut msg_info, false)?;
        }
    } else {
        msg_info.note("this is a dry run. to remove the containers, pass the `--execute` flag.")?;
        for command in commands {
            command.print(&mut msg_info)?;
        }
    }

    Ok(())
}
