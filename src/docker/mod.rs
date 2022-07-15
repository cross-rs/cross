pub mod custom;
mod engine;
mod image;
mod local;
mod provided_images;
pub mod remote;
mod shared;

pub use self::engine::*;
pub use self::provided_images::PROVIDED_IMAGES;
pub use self::shared::*;

pub use image::{Architecture, Image, ImagePlatform, Os as ContainerOs, PossibleImage};

use std::process::ExitStatus;

use crate::errors::*;
use crate::shell::MessageInfo;

#[derive(Debug)]
pub struct ProvidedImage {
    /// The `name` of the image, usually the target triplet
    pub name: &'static str,
    pub platforms: &'static [ImagePlatform],
    pub sub: Option<&'static str>,
}

impl ProvidedImage {
    pub fn image_name(&self, repository: &str, tag: &str) -> String {
        image_name(self.name, self.sub, repository, tag)
    }
}

pub fn image_name(target: &str, sub: Option<&str>, repository: &str, tag: &str) -> String {
    if let Some(sub) = sub {
        format!("{repository}/{target}:{tag}-{sub}")
    } else {
        format!("{repository}/{target}:{tag}")
    }
}

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
