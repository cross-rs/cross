#![deny(missing_debug_implementations, rust_2018_idioms)]

pub mod build_docker_image;
pub mod changelog;
pub mod ci;
pub mod codegen;
pub mod crosstool;
pub mod hooks;
pub mod install_git_hooks;
pub mod target_info;
pub mod util;

use ci::CiJob;
use clap::{CommandFactory, Parser, Subcommand};
use codegen::Codegen;
use cross::docker;
use cross::shell::{MessageInfo, Verbosity};
use util::{cargo_metadata, ImageTarget};

use self::build_docker_image::BuildDockerImage;
use self::changelog::Changelog;
use self::crosstool::ConfigureCrosstool;
use self::hooks::{Check, Test};
use self::install_git_hooks::InstallGitHooks;
use self::target_info::TargetInfo;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Cli {
    /// Toolchain name/version to use (such as stable or 1.59.0).
    #[clap(value_parser = is_toolchain)]
    toolchain: Option<String>,
    #[clap(subcommand)]
    command: Commands,
    /// Provide verbose diagnostic output.
    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
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
    /// Extract and print info for targets.
    TargetInfo(TargetInfo),
    /// Build a docker image from file.
    BuildDockerImage(BuildDockerImage),
    /// Install git development hooks.
    InstallGitHooks(InstallGitHooks),
    /// Run code formatting checks and lints.
    Check(Check),
    /// Run unittest suite.
    Test(Test),
    /// CI tasks
    #[clap(subcommand, hide = true)]
    CiJob(CiJob),
    /// Configure crosstool config files.
    ConfigureCrosstool(ConfigureCrosstool),
    /// Changelog related commands
    #[clap(subcommand)]
    Changelog(Changelog),
    /// Code generation
    Codegen(Codegen),
}

fn is_toolchain(toolchain: &str) -> cross::Result<String> {
    if toolchain.starts_with('+') {
        Ok(toolchain.chars().skip(1).collect())
    } else {
        let _ = <CliHidden as CommandFactory>::command().get_matches();
        unreachable!();
    }
}

macro_rules! get_engine {
    ($args:ident, $msg_info:ident) => {{
        get_container_engine($args.engine.as_deref(), &mut $msg_info)
    }};
}

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let cli = Cli::parse();
    let mut msg_info = MessageInfo::create(cli.verbose, cli.quiet, cli.color.as_deref())?;
    match cli.command {
        Commands::TargetInfo(args) => {
            let engine = get_engine!(args, msg_info)?;
            target_info::target_info(args, &engine, &mut msg_info)?;
        }
        Commands::BuildDockerImage(args) => {
            let engine = get_engine!(args, msg_info)?;
            build_docker_image::build_docker_image(args, &engine, &mut msg_info)?;
        }
        Commands::InstallGitHooks(_) => {
            install_git_hooks::install_git_hooks(&mut msg_info)?;
        }
        Commands::Check(args) => {
            hooks::check(args, cli.toolchain.as_deref(), &mut msg_info)?;
        }
        Commands::Test(args) => {
            hooks::test(args, cli.toolchain.as_deref(), &mut msg_info)?;
        }
        Commands::CiJob(args) => {
            let metadata = cargo_metadata(&mut Verbosity::Verbose(2).into())?;
            ci::ci(args, metadata)?;
        }
        Commands::ConfigureCrosstool(args) => {
            crosstool::configure_crosstool(args, &mut msg_info)?;
        }
        Commands::Changelog(args) => {
            changelog::changelog(args, &mut msg_info)?;
        }
        Commands::Codegen(args) => codegen::codegen(args)?,
    }

    Ok(())
}

fn get_container_engine(
    engine: Option<&str>,
    msg_info: &mut MessageInfo,
) -> cross::Result<docker::Engine> {
    let engine = if let Some(ce) = engine {
        which::which(ce)?
    } else {
        docker::get_container_engine()?
    };
    docker::Engine::from_path(engine, None, None, msg_info)
}
