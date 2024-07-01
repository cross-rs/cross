use clap::Args as ClapArgs;
use cross::config::Config;
use cross::shell::{MessageInfo, Verbosity};
use cross::{
    cargo_metadata_with_args, cli::Args, docker, rustc, setup, toml, CommandVariant, CrossSetup,
    Target,
};
use eyre::Context;

#[derive(ClapArgs, Debug)]
pub struct Run {
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// Target
    #[clap(short, long)]
    pub target: String,
    /// Interactive session
    #[clap(short, long, default_value = "false")]
    pub interactive: bool,
    /// Command to run, will be run in a shell
    #[clap(last = true)]
    pub command: String,
}

impl Run {
    pub fn run(
        &self,
        cli: &crate::Cli,
        engine: docker::Engine,
        msg_info: &mut MessageInfo,
    ) -> cross::Result<()> {
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
            verbose: if cli.verbose { 1 } else { 0 },
            quiet: cli.quiet,
            color: cli.color.clone(),
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
            let config = Config::new(Some(toml));

            let image = match docker::get_image(&config, &target, false) {
                Ok(i) => i,
                Err(docker::GetImageError::NoCompatibleImages(..))
                    if config.dockerfile(&target).is_some() =>
                {
                    "scratch".into()
                }
                Err(err) => {
                    msg_info.warn(&err)?;
                    eyre::bail!("Error: {}", &err);
                }
            };

            let image = image.to_definite_with(&engine, msg_info)?;

            let paths = docker::DockerPaths::create(&engine, metadata, cwd, toolchain, msg_info)?;
            let options = docker::DockerOptions::new(
                engine,
                target,
                config,
                image,
                CommandVariant::Shell,
                None,
                self.interactive,
            );

            let mut args = vec![String::from("-c")];
            args.push(self.command.clone());

            docker::run(options, paths, &args, None, msg_info)
                .wrap_err("could not run container")?;
        }

        Ok(())
    }

    pub fn engine(&self) -> Option<&str> {
        self.engine.as_deref()
    }
}
