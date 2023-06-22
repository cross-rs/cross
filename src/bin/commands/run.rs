use clap::Args as ClapArgs;
use cross::config::Config;
use cross::shell::{MessageInfo, Verbosity};
use cross::SafeCommand;
use cross::{
    cargo_metadata_with_args, cli::Args, docker, rustc, setup, toml, CargoVariant, CrossSetup,
    Target,
};
use eyre::Context;

#[derive(ClapArgs, Debug)]
pub struct Run {
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

    #[clap(short, long)]
    pub target: String,

    #[clap(short, long)]
    pub command: String,
}

impl Run {
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        let target_list = rustc::target_list(&mut Verbosity::Quiet.into())?;
        let target = Target::from(&self.target, &target_list);

        let cwd = std::env::current_dir()?;
        let host_version_meta = rustc::version_meta()?;

        let args = Args {
            cargo_args: vec![],
            rest_args: vec![],
            subcommand: None,
            channel: None,
            target: Some(target.clone()),
            features: vec![],
            target_dir: None,
            manifest_path: None,
            version: false,
            verbose: if self.verbose { 1 } else { 0 },
            quiet: self.quiet,
            color: self.color.clone(),
        };

        if let Some(metadata) = cargo_metadata_with_args(None, Some(&args), msg_info)? {
            let CrossSetup { toolchain, .. } =
                match setup(&host_version_meta, &metadata, &args, target_list, msg_info)? {
                    Some(setup) => setup,
                    _ => {
                        eyre::bail!("Error: cannot setup cross environment");
                    }
                };

            let toml = toml(&metadata, msg_info)?;
            let config = Config::new(toml);

            let image = match docker::get_image(&config, &target, false) {
                Ok(i) => i,
                Err(err) => {
                    msg_info.warn(&err)?;
                    eyre::bail!("Error: {}", &err);
                }
            };

            let image = image.to_definite_with(&engine, msg_info);

            let paths = docker::DockerPaths::create(&engine, metadata, cwd, toolchain, msg_info)?;
            let options =
                docker::DockerOptions::new(engine, target, config, image, CargoVariant::None, None);

            let command = SafeCommand::new("sh");
            let mut args = vec![String::from("-c")];
            args.push(self.command.clone());

            docker::run(options, paths, command, &args, None, msg_info)
                .wrap_err("could not run container")?;
        }

        Ok(())
    }

    pub fn engine(&self) -> Option<&str> {
        self.engine.as_deref()
    }

    pub fn verbose(&self) -> bool {
        self.verbose
    }

    pub fn quiet(&self) -> bool {
        self.quiet
    }

    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }
}
