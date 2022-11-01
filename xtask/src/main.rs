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
use self::changelog::{BuildChangelog, ValidateChangelog};
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
    /// Build the changelog.
    BuildChangelog(BuildChangelog),
    /// Validate changelog entries.
    #[clap(hide = true)]
    ValidateChangelog(ValidateChangelog),
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

macro_rules! get_msg_info {
    ($args:ident, $verbose:expr) => {{
        MessageInfo::create($verbose, $args.quiet, $args.color.as_deref())
    }};
}

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let cli = Cli::parse();
    match cli.command {
        Commands::TargetInfo(args) => {
            let mut msg_info = get_msg_info!(args, args.verbose)?;
            let engine = get_engine!(args, msg_info)?;
            target_info::target_info(args, &engine, &mut msg_info)?;
        }
        Commands::BuildDockerImage(args) => {
            let mut msg_info = get_msg_info!(args, args.verbose != 0)?;
            let engine = get_engine!(args, msg_info)?;
            build_docker_image::build_docker_image(args, &engine, &mut msg_info)?;
        }
        Commands::InstallGitHooks(args) => {
            let mut msg_info = get_msg_info!(args, args.verbose)?;
            install_git_hooks::install_git_hooks(&mut msg_info)?;
        }
        Commands::Check(args) => {
            let mut msg_info = get_msg_info!(args, args.verbose)?;
            hooks::check(args, cli.toolchain.as_deref(), &mut msg_info)?;
        }
        Commands::Test(args) => {
            let mut msg_info = get_msg_info!(args, args.verbose)?;
            hooks::test(args, cli.toolchain.as_deref(), &mut msg_info)?;
        }
        Commands::CiJob(args) => {
            let metadata = cargo_metadata(&mut Verbosity::Verbose(2).into())?;
            ci::ci(args, metadata)?;
        }
        Commands::ConfigureCrosstool(args) => {
            let mut msg_info = get_msg_info!(args, args.verbose)?;
            crosstool::configure_crosstool(args, &mut msg_info)?;
        }
        Commands::BuildChangelog(args) => {
            let mut msg_info = get_msg_info!(args, args.verbose)?;
            changelog::build_changelog(args, &mut msg_info)?;
        }
        Commands::ValidateChangelog(args) => {
            let mut msg_info = get_msg_info!(args, args.verbose)?;
            changelog::validate_changelog(args, &mut msg_info)?;
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
