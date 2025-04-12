mod build;
pub(crate) mod custom;
mod engine;
mod image;
mod local;
mod provided_images;
pub mod remote;
mod shared;

pub use self::build::{BuildCommandExt, BuildResultExt, Progress};
pub use self::engine::*;
pub use self::provided_images::PROVIDED_IMAGES;
pub use self::shared::*;

pub use image::{
    Architecture, Image, ImagePlatform, ImageReference, Os as ContainerOs, PossibleImage,
};

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

    pub fn default_image_name(&self) -> String {
        self.image_name(CROSS_IMAGE, DEFAULT_IMAGE_VERSION)
    }
}

pub fn image_name(target: &str, sub: Option<&str>, repository: &str, tag: &str) -> String {
    if let Some(sub) = sub {
        format!("{repository}/{target}:{tag}-{sub}")
    } else {
        format!("{repository}/{target}:{tag}")
    }
}

// TODO: The Option here in the result should be removed and Result::Error replaced with a enum to properly signal error

// Ok(None) means that the command failed, due to a warning or error, when `msg_info.should_fail() == true`
pub fn run(
    options: DockerOptions,
    paths: DockerPaths,
    args: &[String],
    subcommand: Option<crate::Subcommand>,
    msg_info: &mut MessageInfo,
) -> Result<Option<ExitStatus>> {
    if cfg!(target_os = "windows") && options.in_docker() {
        msg_info.fatal(
            "running cross insider a container running windows is currently unsupported",
            1,
        );
    }
    if options.is_remote() {
        remote::run(options, paths, args, subcommand, msg_info)
            .wrap_err("could not complete remote run")
    } else {
        local::run(options, paths, args, msg_info)
    }
}
