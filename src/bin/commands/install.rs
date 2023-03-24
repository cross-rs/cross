use clap::Args;
use cross::{docker, rustc, shell::MessageInfo};
use eyre::Context;

#[derive(Args, Debug)]
pub struct Install {
    #[clap(long)]
    target: Option<String>,
    #[clap(long)]
    root: String,
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print
    #[clap(short, long)]
    pub quiet: bool,
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// Path to crate
    #[clap(long)]
    pub path: Option<String>,
    /// Path to Cross.toml
    #[clap(long)]
    pub config: Option<std::path::PathBuf>,
    #[clap(name = "crate")]
    krate: String,
}

impl Install {
    pub fn verbose(&self) -> bool {
        self.verbose
    }

    pub fn quiet(&self) -> bool {
        self.quiet
    }

    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    pub fn run(self, msg_info: &mut MessageInfo) -> cross::Result<std::process::ExitStatus> {
        let target_list = rustc::target_list(&mut cross::shell::Verbosity::Quiet.into())?;

        let host_version_meta = rustc::version_meta()?;
        let mut command = vec!["install".to_owned(), self.krate];

        if let Some(target) = self.target {
            command.push(format!("--target={target}"));
        }
        command.push(format!("--root={}", self.root));

        if let Some(engine) = self.engine {
            std::env::set_var(docker::CROSS_CONTAINER_ENGINE_VAR, engine);
        }

        let args = cross::cli::parse(command, &target_list)?;
        let Some(cross::CrossSetup {
            config,
            target,
            uses_xargo,
            uses_zig,
            uses_build_std,
            zig_version,
            toolchain,
            is_remote,
            engine,
            image,
        }) = cross::setup(&host_version_meta, None, &args, target_list, msg_info)? else {
            eyre::bail!("couldn't setup context for cross (see warning)")
        };

        let mut is_nightly = toolchain.channel.contains("nightly");
        let mut rustc_version = None;
        if let Some((version, channel, _)) = toolchain.rustc_version()? {
            is_nightly = channel == rustc_version::Channel::Nightly;
            rustc_version = Some(version);
        }

        let available_targets = cross::rustup::setup_rustup(&toolchain, msg_info)?;

        cross::rustup::setup_components(
            &target,
            uses_xargo,
            uses_build_std,
            &toolchain,
            is_nightly,
            available_targets,
            &args,
            msg_info,
        )?;

        let filtered_args = cross::get_filtered_args(
            zig_version,
            &args,
            &target,
            &config,
            is_nightly,
            uses_build_std,
        );

        let cwd = std::env::current_dir()?;

        let paths = docker::DockerPaths::create(
            &engine,
            cross::CargoMetadata {
                workspace_root: cwd.clone(),
                target_directory: cross::file::absolute_path(self.root)?,
                packages: vec![],
                workspace_members: vec![],
            },
            cwd,
            toolchain.clone(),
            msg_info,
        )?;
        let mut options = docker::DockerOptions::new(
            engine,
            target.clone(),
            config,
            image,
            cross::CargoVariant::create(uses_zig, uses_xargo)?,
            rustc_version,
        );

        options.skip_target_dir = true;

        cross::install_interpreter_if_needed(
            &args,
            host_version_meta,
            &target,
            &options,
            msg_info,
        )?;

        let status = docker::run(options, paths, &filtered_args, msg_info)
            .wrap_err("could not run container")?;
        if !status.success() {
            cross::warn_on_failure(&target, &toolchain, msg_info)?;
        }
        Ok(status)
    }
}
