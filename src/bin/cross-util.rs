#![deny(missing_debug_implementations, rust_2018_idioms)]

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Cli {
    /// Toolchain name/version to use (such as stable or 1.59.0).
    #[clap(value_parser = is_toolchain)]
    toolchain: Option<String>,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List cross images in local storage.
    ListImages(commands::ListImages),
    /// Remove cross images in local storage.
    RemoveImages(commands::RemoveImages),
}

fn is_toolchain(toolchain: &str) -> cross::Result<String> {
    if toolchain.starts_with('+') {
        Ok(toolchain.chars().skip(1).collect())
    } else {
        eyre::bail!("not a toolchain")
    }
}

fn get_container_engine(
    engine: Option<&str>,
    verbose: bool,
) -> cross::Result<cross::docker::Engine> {
    let engine = if let Some(ce) = engine {
        which::which(ce)?
    } else {
        cross::docker::get_container_engine()?
    };
    cross::docker::Engine::from_path(engine, verbose)
}

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let cli = Cli::parse();
    match cli.command {
        Commands::ListImages(args) => {
            let engine = get_container_engine(args.engine.as_deref(), args.verbose)?;
            commands::list_images(args, &engine)?;
        }
        Commands::RemoveImages(args) => {
            let engine = get_container_engine(args.engine.as_deref(), args.verbose)?;
            if args.targets.is_empty() {
                commands::remove_all_images(args, &engine)?;
            } else {
                commands::remove_target_images(args, &engine)?;
            }
        }
    }

    Ok(())
}
