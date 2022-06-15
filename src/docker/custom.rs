use std::io::Write;
use std::path::{Path, PathBuf};

use crate::docker::Engine;
use crate::{config::Config, docker, CargoMetadata, Target};
use crate::{errors::*, file, CommandExt};

use super::{image_name, parse_docker_opts};

#[derive(Debug, PartialEq, Eq)]
pub enum Dockerfile<'a> {
    File {
        path: &'a str,
        context: Option<&'a str>,
        name: Option<&'a str>,
    },
    Custom {
        content: String,
    },
}

impl<'a> Dockerfile<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &self,
        config: &Config,
        metadata: &CargoMetadata,
        engine: &Engine,
        host_root: &Path,
        build_args: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
        target_triple: &Target,
        verbose: bool,
    ) -> Result<String> {
        let mut docker_build = docker::subcommand(engine, "build");
        docker_build.current_dir(host_root);
        docker_build.env("DOCKER_SCAN_SUGGEST", "false");
        docker_build.args([
            "--label",
            &format!(
                "{}.for-cross-target={target_triple}",
                crate::CROSS_LABEL_DOMAIN
            ),
        ]);

        docker_build.args([
            "--label",
            &format!(
                "{}.workspace_root={}",
                crate::CROSS_LABEL_DOMAIN,
                metadata.workspace_root.display()
            ),
        ]);

        let image_name = self.image_name(target_triple, metadata);
        docker_build.args(["--tag", &image_name]);

        for (key, arg) in build_args.into_iter() {
            docker_build.args(["--build-arg", &format!("{}={}", key.as_ref(), arg.as_ref())]);
        }

        if let Some(arch) = target_triple.deb_arch() {
            docker_build.args(["--build-arg", &format!("CROSS_DEB_ARCH={arch}")]);
        }

        let path = match self {
            Dockerfile::File { path, .. } => PathBuf::from(path),
            Dockerfile::Custom { content } => {
                let path = metadata
                    .target_directory
                    .join(target_triple.to_string())
                    .join(format!("Dockerfile.{}-custom", target_triple,));
                {
                    let mut file = file::write_file(&path, true)?;
                    file.write_all(content.as_bytes())?;
                }
                path
            }
        };

        if matches!(self, Dockerfile::File { .. }) {
            if let Ok(cross_base_image) = self::image_name(config, target_triple) {
                docker_build.args([
                    "--build-arg",
                    &format!("CROSS_BASE_IMAGE={cross_base_image}"),
                ]);
            }
        }

        docker_build.args(["--file".into(), path]);

        if let Ok(build_opts) = std::env::var("CROSS_BUILD_OPTS") {
            // FIXME: Use shellwords
            docker_build.args(parse_docker_opts(&build_opts)?);
        }
        if let Some(context) = self.context() {
            docker_build.arg(&context);
        } else {
            docker_build.arg(".");
        }

        docker_build.run(verbose, true)?;
        Ok(image_name)
    }

    pub fn image_name(&self, target_triple: &Target, metadata: &CargoMetadata) -> String {
        match self {
            Dockerfile::File {
                name: Some(name), ..
            } => name.to_string(),
            _ => format!(
                "cross-custom-{package_name}:{target_triple}-{path_hash}{custom}",
                package_name = metadata
                    .workspace_root
                    .file_name()
                    .expect("workspace_root can't end in `..`")
                    .to_string_lossy(),
                path_hash =
                    sha1_smol::Sha1::from(&metadata.workspace_root.to_string_lossy().as_bytes())
                        .digest()
                        .to_string()
                        .get(..5)
                        .expect("sha1 is expected to be at least 5 characters long"),
                custom = if matches!(self, Self::File { .. }) {
                    ""
                } else {
                    "-pre-build"
                }
            ),
        }
    }

    fn context(&self) -> Option<&'a str> {
        match self {
            Dockerfile::File {
                context: Some(context),
                ..
            } => Some(context),
            _ => None,
        }
    }
}
