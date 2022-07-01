use std::io::Write;
use std::path::{Path, PathBuf};

use crate::docker::Engine;
use crate::shell::MessageInfo;
use crate::{config::Config, docker, CargoMetadata, Target};
use crate::{errors::*, file, CommandExt, ToUtf8};

use super::{image_name, parse_docker_opts, path_hash};

pub const CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX: &str = "cross-custom-";

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
        msg_info: MessageInfo,
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
                metadata.workspace_root.to_utf8()?
            ),
        ]);

        let image_name = self.image_name(target_triple, metadata)?;
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

        docker_build.run(msg_info, true)?;
        Ok(image_name)
    }

    pub fn image_name(&self, target_triple: &Target, metadata: &CargoMetadata) -> Result<String> {
        match self {
            Dockerfile::File {
                name: Some(name), ..
            } => Ok(name.to_string()),
            _ => Ok(format!(
                "{}{package_name}:{target_triple}-{path_hash}{custom}",
                CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX,
                package_name = docker_package_name(metadata),
                path_hash = path_hash(&metadata.workspace_root)?,
                custom = if matches!(self, Self::File { .. }) {
                    ""
                } else {
                    "-pre-build"
                }
            )),
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

fn docker_package_name(metadata: &CargoMetadata) -> String {
    // a valid image name consists of the following:
    // - lowercase ASCII letters
    // - digits
    // - a period
    // - 1-2 underscores
    // - 1 or more hyphens (dashes)
    docker_tag_name(
        &metadata
            .workspace_root
            .file_name()
            .expect("workspace_root can't end in `..`")
            .to_string_lossy(),
    )
}

fn docker_tag_name(file_name: &str) -> String {
    // a valid image name consists of the following:
    // - lowercase ASCII letters
    // - digits
    // - a period
    // - 1-2 underscores
    // - 1 or more hyphens (dashes)
    let mut result = String::new();
    let mut consecutive_underscores = 0;
    for c in file_name.chars() {
        match c {
            'a'..='z' | '.' | '-' => {
                consecutive_underscores = 0;
                result.push(c);
            }
            'A'..='Z' => {
                consecutive_underscores = 0;
                result.push(c.to_ascii_lowercase());
            }
            '_' => {
                consecutive_underscores += 1;
                if consecutive_underscores <= 2 {
                    result.push(c);
                }
            }
            // ignore any non-ascii characters
            _ => (),
        }
    }

    // in case all characters were invalid, use a non-empty filename
    if result.is_empty() {
        result = "empty".to_string();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! s {
        ($s:literal) => {
            $s.to_string()
        };
    }

    #[test]
    fn docker_tag_name_test() {
        assert_eq!(docker_tag_name("package"), s!("package"));
        assert_eq!(docker_tag_name("pAcKaGe"), s!("package"));
        assert_eq!(
            docker_tag_name("package_안녕하세요_test"),
            s!("package__test")
        );
        assert_eq!(
            docker_tag_name("pAcKaGe___test_name"),
            s!("package__test_name")
        );
        assert_eq!(
            docker_tag_name("pAcKaGe---test.name"),
            s!("package---test.name")
        );
    }
}
