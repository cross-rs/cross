#![deny(missing_debug_implementations, rust_2018_idioms)]

pub mod build_docker_image;
pub mod target_info;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use self::build_docker_image::BuildDockerImage;
use self::target_info::TargetInfo;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Extract and print info for targets.
    TargetInfo(TargetInfo),
    BuildDockerImage(BuildDockerImage),
}

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let cli = Cli::parse();
    match cli.command {
        Commands::TargetInfo(args) => {
            let engine = get_container_engine(args.engine.as_deref())?;
            target_info::target_info(args, &engine)?;
        }
        Commands::BuildDockerImage(args) => {
            let engine = get_container_engine(args.engine.as_deref())?;
            build_docker_image::build_docker_image(args, &engine)?;
        }
    }

    Ok(())
}

fn get_container_engine(engine: Option<&str>) -> Result<PathBuf, which::Error> {
    if let Some(ce) = engine {
        which::which(ce)
    } else {
        cross::get_container_engine()
    }
}
