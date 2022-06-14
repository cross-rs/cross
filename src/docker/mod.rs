mod engine;
mod local;
mod shared;

pub use self::engine::*;
pub use self::shared::*;

use std::path::Path;
use std::process::ExitStatus;

use crate::cargo::CargoMetadata;
use crate::errors::*;
use crate::{Config, Target};

#[allow(clippy::too_many_arguments)] // TODO: refactor
pub fn run(
    target: &Target,
    args: &[String],
    metadata: &CargoMetadata,
    config: &Config,
    uses_xargo: bool,
    sysroot: &Path,
    verbose: bool,
    docker_in_docker: bool,
    cwd: &Path,
) -> Result<ExitStatus> {
    local::run(
        target,
        args,
        metadata,
        config,
        uses_xargo,
        sysroot,
        verbose,
        docker_in_docker,
        cwd,
    )
}
