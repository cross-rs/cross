use std::io;

use clap::{Args, Subcommand};
use cross::docker::ImagePlatform;
use cross::rustc::{QualifiedToolchain, Toolchain};
use cross::shell::{MessageInfo, Stream};
use cross::{docker, CommandExt, TargetTriple};

#[derive(Args, Debug)]
pub struct ListVolumes {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl ListVolumes {
    pub fn run(self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        list_volumes(&engine, msg_info)
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
    /// Coloring: auto, always, never
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
    pub fn run(self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        remove_all_volumes(self, &engine, msg_info)
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
    /// Coloring: auto, always, never
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
    pub fn run(self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        prune_volumes(self, &engine, msg_info)
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
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// Toolchain to create a volume for
    #[clap(long, default_value = TargetTriple::DEFAULT.triple(), )]
    pub toolchain: String,
}

impl CreateVolume {
    pub fn run(
        self,
        engine: docker::Engine,
        channel: Option<&Toolchain>,
        msg_info: &mut MessageInfo,
    ) -> cross::Result<()> {
        create_persistent_volume(self, &engine, channel, msg_info)
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
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// Toolchain to remove the volume for
    #[clap(long, default_value = TargetTriple::DEFAULT.triple(), )]
    pub toolchain: String,
}

impl RemoveVolume {
    pub fn run(
        self,
        engine: docker::Engine,
        channel: Option<&Toolchain>,
        msg_info: &mut MessageInfo,
    ) -> cross::Result<()> {
        remove_persistent_volume(self, &engine, channel, msg_info)
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
    /// Create a persistent data volume for a given toolchain.
    Create(CreateVolume),
    /// Remove a persistent data volume for a given toolchain.
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
    pub fn run(
        self,
        engine: docker::Engine,
        channel: Option<&Toolchain>,
        msg_info: &mut MessageInfo,
    ) -> cross::Result<()> {
        match self {
            Volumes::List(args) => args.run(engine, msg_info),
            Volumes::RemoveAll(args) => args.run(engine, msg_info),
            Volumes::Prune(args) => args.run(engine, msg_info),
            Volumes::Create(args) => args.run(engine, channel, msg_info),
            Volumes::Remove(args) => args.run(engine, channel, msg_info),
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
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl ListContainers {
    pub fn run(self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        list_containers(&engine, msg_info)
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
    /// Coloring: auto, always, never
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
    pub fn run(self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        remove_all_containers(self, &engine, msg_info)
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
    pub fn run(self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        match self {
            Containers::List(args) => args.run(engine, msg_info),
            Containers::RemoveAll(args) => args.run(engine, msg_info),
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
    use cross::docker::remote::VOLUME_PREFIX;
    let stdout = docker::subcommand(engine, "volume")
        .arg("list")
        .args(["--format", "{{.Name}}"])
        // handles simple regex: ^ for start of line.
        .args(["--filter", &format!("name=^{VOLUME_PREFIX}")])
        .run_and_get_stdout(msg_info)?;

    let mut volumes: Vec<String> = stdout.lines().map(|s| s.to_string()).collect();
    volumes.sort();

    Ok(volumes)
}

pub fn list_volumes(engine: &docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
    for line in get_cross_volumes(engine, msg_info)?.iter() {
        msg_info.print(line)?;
    }

    Ok(())
}

pub fn remove_all_volumes(
    RemoveAllVolumes { force, execute, .. }: RemoveAllVolumes,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let volumes = get_cross_volumes(engine, msg_info)?;

    let mut command = docker::subcommand(engine, "volume");
    command.arg("rm");
    if force {
        command.arg("--force");
    }
    command.args(&volumes);
    if volumes.is_empty() {
        Ok(())
    } else if execute {
        command.run(msg_info, false).map_err(Into::into)
    } else {
        msg_info.note("this is a dry run. to remove the volumes, pass the `--execute` flag.")?;
        command.print(msg_info)?;
        Ok(())
    }
}

pub fn prune_volumes(
    PruneVolumes { execute, .. }: PruneVolumes,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut command = docker::subcommand(engine, "volume");
    command.args(["prune", "--force"]);
    if execute {
        command.run(msg_info, false).map_err(Into::into)
    } else {
        msg_info.note("this is a dry run. to prune the volumes, pass the `--execute` flag.")?;
        command.print(msg_info)?;
        Ok(())
    }
}

pub fn create_persistent_volume(
    CreateVolume {
        copy_registry,
        toolchain,
        ..
    }: CreateVolume,
    engine: &docker::Engine,
    channel: Option<&Toolchain>,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut toolchain = toolchain_or_target(&toolchain, msg_info)?;
    if let Some(channel) = channel {
        toolchain.channel = channel.channel.clone();
    };
    let mount_finder = docker::MountFinder::create(engine)?;
    let dirs = docker::ToolchainDirectories::assemble(&mount_finder, toolchain.clone())?;
    let container = dirs.unique_container_identifier(&toolchain.host().target)?;
    let volume = dirs.unique_toolchain_identifier()?;

    if docker::remote::volume_exists(engine, &volume, msg_info)? {
        eyre::bail!("Error: volume {volume} already exists.");
    }

    docker::subcommand(engine, "volume")
        .args(["create", &volume])
        .run_and_get_status(msg_info, false)?;

    // stop the container if it's already running
    let state = docker::remote::container_state(engine, &container, msg_info)?;
    if !state.is_stopped() {
        msg_info.warn(format_args!("container {container} was running."))?;
        docker::remote::container_stop_default(engine, &container, msg_info)?;
    }
    if state.exists() {
        msg_info.warn(format_args!("container {container} was exited."))?;
        docker::remote::container_rm(engine, &container, msg_info)?;
    }

    // create a dummy running container to copy data over
    let mount_prefix = docker::remote::MOUNT_PREFIX;
    let mut docker = docker::subcommand(engine, "run");
    docker.args(["--name", &container]);
    docker.arg("--rm");
    docker.args(["-v", &format!("{}:{}", volume, mount_prefix)]);
    docker.arg("-d");
    let is_tty = io::Stdin::is_atty() && io::Stdout::is_atty() && io::Stderr::is_atty();
    if is_tty {
        docker.arg("-t");
    }
    docker.arg(docker::UBUNTU_BASE);
    if !is_tty {
        // ensure the process never exits until we stop it
        // we only need this infinite loop if we don't allocate
        // a TTY. this has a few issues though: now, the
        // container no longer responds to signals, so the
        // container will need to be sig-killed.
        docker.args(["sh", "-c", "sleep infinity"]);
    }
    // store first, since failing to non-existing container is fine
    docker::remote::create_container_deleter(engine.clone(), container.clone());
    docker.run_and_get_status(msg_info, false)?;

    docker::remote::copy_volume_container_xargo(
        engine,
        &container,
        &dirs,
        mount_prefix.as_ref(),
        msg_info,
    )?;
    docker::remote::copy_volume_container_cargo(
        engine,
        &container,
        &dirs,
        mount_prefix.as_ref(),
        copy_registry,
        msg_info,
    )?;
    docker::remote::copy_volume_container_rust(
        engine,
        &container,
        &dirs,
        None,
        mount_prefix.as_ref(),
        msg_info,
    )?;

    docker::remote::drop_container(is_tty, msg_info);

    Ok(())
}

pub fn remove_persistent_volume(
    RemoveVolume { toolchain, .. }: RemoveVolume,
    engine: &docker::Engine,
    channel: Option<&Toolchain>,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut toolchain = toolchain_or_target(&toolchain, msg_info)?;
    if let Some(channel) = channel {
        toolchain.channel = channel.channel.clone();
    };
    let mount_finder = docker::MountFinder::create(engine)?;
    let dirs = docker::ToolchainDirectories::assemble(&mount_finder, toolchain)?;
    let volume = dirs.unique_toolchain_identifier()?;

    if !docker::remote::volume_exists(engine, &volume, msg_info)? {
        eyre::bail!("Error: volume {volume} does not exist.");
    }

    docker::remote::volume_rm(engine, &volume, msg_info)?;

    Ok(())
}

fn get_cross_containers(
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<Vec<String>> {
    use cross::docker::remote::VOLUME_PREFIX;
    let stdout = docker::subcommand(engine, "ps")
        .arg("-a")
        .args(["--format", "{{.Names}}: {{.State}}"])
        // handles simple regex: ^ for start of line.
        .args(["--filter", &format!("name=^{VOLUME_PREFIX}")])
        .run_and_get_stdout(msg_info)?;

    let mut containers: Vec<String> = stdout.lines().map(|s| s.to_string()).collect();
    containers.sort();

    Ok(containers)
}

pub fn list_containers(engine: &docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
    for line in get_cross_containers(engine, msg_info)?.iter() {
        msg_info.print(line)?;
    }

    Ok(())
}

pub fn remove_all_containers(
    RemoveAllContainers { force, execute, .. }: RemoveAllContainers,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let containers = get_cross_containers(engine, msg_info)?;
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
            command.run(msg_info, false)?;
        }
    } else {
        msg_info.note("this is a dry run. to remove the containers, pass the `--execute` flag.")?;
        for command in commands {
            command.print(msg_info)?;
        }
    }

    Ok(())
}

fn toolchain_or_target(
    s: &str,
    msg_info: &mut MessageInfo,
) -> Result<QualifiedToolchain, color_eyre::Report> {
    let config = cross::config::Config::new(None);
    let mut toolchain = QualifiedToolchain::default(&config, msg_info)?;
    let target_list = cross::rustc::target_list(msg_info)?;
    if target_list.contains(s) {
        toolchain.replace_host(&ImagePlatform::from_target(s.into())?);
    } else {
        let picked: Toolchain = s.parse()?;
        toolchain = toolchain.with_picked(picked)?;
    }

    Ok(toolchain)
}
