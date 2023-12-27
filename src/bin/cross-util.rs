#![deny(missing_debug_implementations, rust_2018_idioms)]

use clap::{CommandFactory, Parser, Subcommand};
use cross::shell::MessageInfo;
use cross::{docker, rustc::Toolchain};

mod commands;

const APP_NAME: &str = "cross-util";
static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), cross::commit_info!());

#[derive(Parser, Debug)]
#[clap(about, long_about = None, name = APP_NAME, version = VERSION)]
pub struct Cli {
    /// Toolchain name/version to use (such as stable or 1.59.0).
    #[clap(value_parser = is_toolchain)]
    toolchain: Option<Toolchain>,
    #[clap(subcommand)]
    command: Commands,
    /// Provide verbose diagnostic output.
    #[clap(short, long, global = true)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long, global = true)]
    pub quiet: bool,
    /// Coloring: auto, always, never
    #[clap(long, global = true)]
    pub color: Option<String>,
}

// hidden implied parser so we can get matches without recursion.
#[derive(Parser, Debug)]
struct CliHidden {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Work with cross images in local storage.
    #[clap(subcommand)]
    Images(commands::Images),
    /// Work with cross volumes in local storage.
    #[clap(subcommand)]
    Volumes(commands::Volumes),
    /// Work with cross containers in local storage.
    #[clap(subcommand)]
    Containers(commands::Containers),
    /// Run in cross container.
    Run(commands::Run),
    /// Clean all cross data in local storage.
    Clean(commands::Clean),
}

fn is_toolchain(toolchain: &str) -> cross::Result<Toolchain> {
    if toolchain.starts_with('+') {
        Ok(toolchain.chars().skip(1).collect::<String>().parse()?)
    } else {
        let _ = <CliHidden as CommandFactory>::command().get_matches();
        unreachable!();
    }
}

fn get_container_engine(
    engine: Option<&str>,
    docker_in_docker: bool,
    msg_info: &mut MessageInfo,
) -> cross::Result<docker::Engine> {
    let engine = if let Some(ce) = engine {
        which::which(ce)?
    } else {
        docker::get_container_engine()?
    };
    let in_docker = match docker_in_docker {
        true => Some(true),
        false => None,
    };
    docker::Engine::from_path(engine, in_docker, None, msg_info)
}

macro_rules! get_engine {
    ($args:ident, $docker_in_docker:expr, $msg_info: ident) => {{
        get_container_engine($args.engine(), $docker_in_docker, &mut $msg_info)
    }};
}

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let cli = Cli::parse();
    let mut msg_info = MessageInfo::create(cli.verbose, cli.quiet, cli.color.as_deref())?;
    match &cli.command {
        Commands::Images(args) => {
            let engine = get_engine!(args, false, msg_info)?;
            args.run(engine, &mut msg_info)?;
        }
        Commands::Volumes(args) => {
            let engine = get_engine!(args, args.docker_in_docker(), msg_info)?;
            args.run(engine, cli.toolchain.as_ref(), &mut msg_info)?;
        }
        Commands::Containers(args) => {
            let engine = get_engine!(args, false, msg_info)?;
            args.run(engine, &mut msg_info)?;
        }
        Commands::Clean(args) => {
            let engine = get_engine!(args, false, msg_info)?;
            args.run(engine, &mut msg_info)?;
        }
        Commands::Run(args) => {
            let engine = get_engine!(args, false, msg_info)?;
            args.run(&cli, engine, &mut msg_info)?;
        }
    }

    Ok(())
}
