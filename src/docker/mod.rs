pub mod custom;
mod engine;
mod local;
pub mod remote;
mod shared;

pub use self::engine::*;
pub use self::shared::*;

use std::process::ExitStatus;

use crate::errors::*;
use crate::shell::MessageInfo;

pub fn run(
    options: DockerOptions,
    paths: DockerPaths,
    args: &[String],
    msg_info: &mut MessageInfo,
) -> Result<ExitStatus> {
    if cfg!(target_os = "windows") && options.in_docker() {
        msg_info.fatal(
            "running cross insider a container running windows is currently unsupported",
            1,
        );
    }
    if options.is_remote() {
        remote::run(options, paths, args, msg_info).wrap_err("could not complete remote run")
    } else {
        local::run(options, paths, args, msg_info)
    }
}
