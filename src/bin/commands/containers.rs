use std::io;

use clap::{Args, Subcommand};
use cross::docker::ImagePlatform;
use cross::rustc::{QualifiedToolchain, Toolchain};
use cross::shell::{MessageInfo, Stream};
use cross::{docker, CommandExt, TargetTriple};

#[derive(Args, Debug)]
pub struct ListVolumes {
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl ListVolumes {
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        list_volumes(&engine, msg_info)
    }
}

#[derive(Args, Debug)]
pub struct RemoveAllVolumes {
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
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        remove_all_volumes(self, &engine, msg_info)
    }
}

#[derive(Args, Debug)]
pub struct PruneVolumes {
    /// Remove volumes. Default is a dry run.
    #[clap(short, long)]
    pub execute: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl PruneVolumes {
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
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
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// Toolchain to create a volume for
    #[clap(long, default_value = TargetTriple::DEFAULT.triple(), )]
    pub toolchain: String,
}

impl CreateVolume {
    pub fn run(
        &self,
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
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// Toolchain to remove the volume for
    #[clap(long, default_value = TargetTriple::DEFAULT.triple(), )]
    pub toolchain: String,
}

impl RemoveVolume {
    pub fn run(
        &self,
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
impl Volumes {
    pub fn run(
        &self,
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
        match self {
            Volumes::List(l) => l.engine.as_deref(),
            Volumes::RemoveAll(l) => l.engine.as_deref(),
            Volumes::Prune(l) => l.engine.as_deref(),
            Volumes::Create(l) => l.engine.as_deref(),
            Volumes::Remove(l) => l.engine.as_deref(),
        }
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
}

#[derive(Args, Debug)]
pub struct ListContainers {
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl ListContainers {
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        list_containers(&engine, msg_info)
    }
}

#[derive(Args, Debug)]
pub struct RemoveAllContainers {
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
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
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

impl Containers {
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        match self {
            Containers::List(args) => args.run(engine, msg_info),
            Containers::RemoveAll(args) => args.run(engine, msg_info),
        }
    }

    pub fn engine(&self) -> Option<&str> {
        match self {
            Containers::List(l) => l.engine.as_deref(),
            Containers::RemoveAll(l) => l.engine.as_deref(),
        }
    }
}

fn get_cross_volumes(
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<Vec<String>> {
    use cross::docker::VOLUME_PREFIX;
    let stdout = engine
        .subcommand("volume")
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
    RemoveAllVolumes { force, execute, .. }: &RemoveAllVolumes,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let volumes = get_cross_volumes(engine, msg_info)?;

    let mut command = engine.subcommand("volume");
    command.arg("rm");
    if *force {
        command.arg("--force");
    }
    command.args(&volumes);
    if volumes.is_empty() {
        Ok(())
    } else if *execute {
        command.run(msg_info, false)
    } else {
        msg_info.note("this is a dry run. to remove the volumes, pass the `--execute` flag.")?;
        command.print(msg_info)?;
        Ok(())
    }
}

pub fn prune_volumes(
    PruneVolumes { execute, .. }: &PruneVolumes,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut command = engine.subcommand("volume");
    command.args(["prune", "--force"]);
    if *execute {
        command.run(msg_info, false)
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
    }: &CreateVolume,
    engine: &docker::Engine,
    channel: Option<&Toolchain>,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut toolchain = toolchain_or_target(toolchain, msg_info)?;
    if let Some(channel) = channel {
        toolchain.channel = channel.channel.clone();
    };
    let mount_finder = docker::MountFinder::create(engine, msg_info)?;
    let dirs = docker::ToolchainDirectories::assemble(&mount_finder, toolchain.clone())?;
    let container_id = dirs.unique_container_identifier(&toolchain.host().target)?;
    let volume_id = dirs.unique_toolchain_identifier()?;
    let volume = docker::DockerVolume::new(engine, &volume_id);

    if volume.exists(msg_info)? {
        eyre::bail!("Error: volume {volume_id} already exists.");
    }

    volume.create(msg_info)?;

    // stop the container if it's already running
    let container = docker::DockerContainer::new(engine, &container_id);
    let state = container.state(msg_info)?;
    if !state.is_stopped() {
        msg_info.warn(format_args!("container {container_id} was running."))?;
        container.stop_default(msg_info)?;
    }
    if state.exists() {
        msg_info.warn(format_args!("container {container_id} was exited."))?;
        container.remove(msg_info)?;
    }

    // create a dummy running container to copy data over
    let mount_prefix = docker::MOUNT_PREFIX;
    let mut docker = engine.subcommand("run");
    docker.args(["--name", &container_id]);
    docker.arg("--rm");
    docker.args(["-v", &format!("{}:{}", volume_id, mount_prefix)]);
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
    docker::ChildContainer::create(engine.clone(), container_id.clone())?;
    docker.run_and_get_status(msg_info, true)?;

    let data_volume = docker::ContainerDataVolume::new(engine, &container_id, &dirs);
    data_volume.copy_xargo(mount_prefix, msg_info)?;
    data_volume.copy_cargo(mount_prefix, *copy_registry, msg_info)?;
    data_volume.copy_rust(None, mount_prefix, msg_info)?;

    docker::ChildContainer::finish_static(is_tty, msg_info);

    Ok(())
}

pub fn remove_persistent_volume(
    RemoveVolume { toolchain, .. }: &RemoveVolume,
    engine: &docker::Engine,
    channel: Option<&Toolchain>,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let mut toolchain = toolchain_or_target(toolchain, msg_info)?;
    if let Some(channel) = channel {
        toolchain.channel = channel.channel.clone();
    };
    let mount_finder = docker::MountFinder::create(engine, msg_info)?;
    let dirs = docker::ToolchainDirectories::assemble(&mount_finder, toolchain)?;
    let volume_id = dirs.unique_toolchain_identifier()?;
    let volume = docker::DockerVolume::new(engine, &volume_id);

    if !volume.exists(msg_info)? {
        eyre::bail!("Error: volume {volume_id} does not exist.");
    }

    volume.remove(msg_info)?;

    Ok(())
}

fn get_cross_containers(
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<Vec<String>> {
    use cross::docker::VOLUME_PREFIX;
    let stdout = engine
        .subcommand("ps")
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
    RemoveAllContainers { force, execute, .. }: &RemoveAllContainers,
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
        let state = docker::ContainerState::new(state.trim())?;
        if state.is_stopped() {
            stopped.push(name);
        } else {
            running.push(name);
        }
    }

    let mut commands = vec![];
    if !running.is_empty() {
        let mut stop = engine.subcommand("stop");
        stop.args(&running);
        commands.push(stop);
    }

    if !(stopped.is_empty() && running.is_empty()) {
        let mut rm = engine.subcommand("rm");
        if *force {
            rm.arg("--force");
        }
        rm.args(&running);
        rm.args(&stopped);
        commands.push(rm);
    }
    if *execute {
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
