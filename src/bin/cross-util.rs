#![deny(missing_debug_implementations, rust_2018_idioms)]

use clap::{CommandFactory, Parser, Subcommand};
use cross::shell::MessageInfo;
use cross::{docker, rustc::Toolchain};

mod commands;

const APP_NAME: &str = "cross-util";
static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), cross::commit_info!());

#[derive(Parser, Debug)]
#[clap(about, long_about = None, name = APP_NAME, version = VERSION)]
struct Cli {
    /// Toolchain name/version to use (such as stable or 1.59.0).
    #[clap(value_parser = is_toolchain)]
    toolchain: Option<Toolchain>,
    #[clap(subcommand)]
    command: Commands,
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

macro_rules! get_msg_info {
    ($args:ident) => {{
        MessageInfo::create($args.verbose(), $args.quiet(), $args.color())
    }};
}

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let cli = Cli::parse();
    match cli.command {
        Commands::Images(args) => {
            let mut msg_info = get_msg_info!(args)?;
            let engine = get_engine!(args, false, msg_info)?;
            args.run(engine, &mut msg_info)?;
        }
        Commands::Volumes(args) => {
            let mut msg_info = get_msg_info!(args)?;
            let engine = get_engine!(args, args.docker_in_docker(), msg_info)?;
            args.run(engine, cli.toolchain.as_ref(), &mut msg_info)?;
        }
        Commands::Containers(args) => {
            let mut msg_info = get_msg_info!(args)?;
            let engine = get_engine!(args, false, msg_info)?;
            args.run(engine, &mut msg_info)?;
        }
        Commands::Clean(args) => {
            let mut msg_info = get_msg_info!(args)?;
            let engine = get_engine!(args, false, msg_info)?;
            args.run(engine, &mut msg_info)?;
        }
    }

    Ok(())
}
