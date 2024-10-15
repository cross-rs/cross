use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use crate::docker::{self, DockerOptions, DockerPaths};
use crate::shell::MessageInfo;
use crate::{errors::*, file, CommandExt, ToUtf8};
use crate::{CargoMetadata, TargetTriple};

use super::{
    create_target_dir, get_image_name, path_hash, BuildCommandExt, BuildResultExt, Engine,
    ImagePlatform,
};

pub const CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX: &str = "localhost/cross-rs/cross-custom-";

#[derive(Debug, PartialEq, Eq)]
pub enum Dockerfile<'a> {
    File {
        path: &'a str,
        context: Option<&'a str>,
        name: Option<&'a str>,
        runs_with: &'a ImagePlatform,
    },
    Custom {
        content: String,
        runs_with: &'a ImagePlatform,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub enum PreBuild {
    /// A path to a file to copy or a single line to `RUN` if line comes from env
    Single { line: String, env: bool },
    /// Lines to execute in a single `RUN`
    Lines(Vec<String>),
}

impl serde::Serialize for PreBuild {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            PreBuild::Single { line, .. } => serializer.serialize_str(line),
            PreBuild::Lines(lines) => {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(lines.len()))?;
                for line in lines {
                    seq.serialize_element(line)?;
                }
                seq.end()
            }
        }
    }
}
impl FromStr for PreBuild {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PreBuild::Single {
            line: s.to_owned(),
            env: false,
        })
    }
}

impl From<Vec<String>> for PreBuild {
    fn from(vec: Vec<String>) -> Self {
        PreBuild::Lines(vec)
    }
}

impl PreBuild {
    #[must_use]
    pub fn is_single(&self) -> bool {
        matches!(self, Self::Single { .. })
    }

    #[must_use]
    pub fn is_lines(&self) -> bool {
        matches!(self, Self::Lines(..))
    }
}

impl<'a> Dockerfile<'a> {
    pub fn build(
        &self,
        options: &DockerOptions,
        paths: &DockerPaths,
        build_args: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
        msg_info: &mut MessageInfo,
    ) -> Result<String> {
        let uses_zig = options.command_variant.uses_zig();
        let mut docker_build = options.engine.command();
        docker_build.invoke_build_command();
        docker_build.disable_scan_suggest();
        self.runs_with()
            .specify_platform(&options.engine, &mut docker_build);

        docker_build.progress(None)?;
        docker_build.verbose(msg_info.verbosity);
        docker_build.cross_labels(options.target.triple(), self.runs_with().target.triple());

        docker_build.args([
            "--label",
            &format!(
                "{}.workspace_root={}",
                crate::CROSS_LABEL_DOMAIN,
                paths.workspace_root().to_utf8()?
            ),
        ]);

        let image_name = self.image_name(options.target.target(), &paths.metadata)?;
        docker_build.args(["--tag", &image_name]);

        for (key, arg) in build_args {
            docker_build.args(["--build-arg", &format!("{}={}", key.as_ref(), arg.as_ref())]);
        }

        if let Some(arch) = options.target.target().deb_arch() {
            docker_build.args(["--build-arg", &format!("CROSS_DEB_ARCH={arch}")]);
        }

        let path = match self {
            Dockerfile::File { path, .. } => {
                paths.metadata.workspace_root.join(PathBuf::from(path))
            }
            Dockerfile::Custom { content, .. } => {
                let target_dir = paths
                    .metadata
                    .target_directory
                    .join(options.target.to_string());
                create_target_dir(&target_dir)?;
                let path = target_dir.join(format!("Dockerfile.{}-custom", &options.target));
                {
                    let mut file = file::write_file(&path, true)?;
                    file.write_all(content.as_bytes())?;
                }
                path
            }
        };

        if matches!(self, Dockerfile::File { .. }) {
            if let Ok(cross_base_image) =
                self::get_image_name(&options.config, &options.target, uses_zig)
            {
                docker_build.args([
                    "--build-arg",
                    &format!("CROSS_BASE_IMAGE={cross_base_image}"),
                ]);
            }
        }

        docker_build.args(["--file".into(), path]);

        if let Some(build_opts) = options.config.build_opts() {
            docker_build.args(Engine::parse_opts(&build_opts)?);
        }

        let has_output = options.config.build_opts().map_or(false, |opts| {
            opts.contains("--load") || opts.contains("--output")
        });
        if options.engine.kind.is_docker() && !has_output {
            docker_build.args(["--output", "type=docker"]);
        };

        if let Some(context) = self.context() {
            docker_build.arg(context);
        } else {
            docker_build.arg(paths.host_root());
        }

        // FIXME: Inspect the error message, while still inheriting stdout on verbose mode to
        // conditionally apply this suggestion and note. This could then inspect if a help string is emitted,
        // if the daemon is not running, etc.
        docker_build
            .run(msg_info, true)
            .engine_warning(&options.engine)
            .buildkit_warning()?;
        Ok(image_name)
    }

    pub fn image_name(
        &self,
        target_triple: &TargetTriple,
        metadata: &CargoMetadata,
    ) -> Result<String> {
        match self {
            Dockerfile::File {
                name: Some(name), ..
            } => Ok((*name).to_owned()),
            _ => Ok(format!(
                "{}{package_name}:{target_triple}-{path_hash}{custom}",
                CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX,
                package_name = docker_package_name(metadata),
                path_hash = path_hash(&metadata.workspace_root, docker::PATH_HASH_SHORT)?,
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
    fn runs_with(&self) -> &ImagePlatform {
        match self {
            Dockerfile::File { runs_with, .. } => runs_with,
            Dockerfile::Custom { runs_with, .. } => runs_with,
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
            'a'..='z' | '0'..='9' | '.' | '-' => {
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

    // in case our result ends in an invalid last char `-` or `.`
    // we remove
    result = result.trim_end_matches(&['.', '-']).to_owned();

    // in case all characters were invalid or we had all non-ASCII
    // characters followed by a `-` or `.`, we use a non-empty filename
    if result.is_empty() {
        result = "empty".to_owned();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! s {
        ($s:literal) => {
            $s.to_owned()
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

        assert_eq!(docker_tag_name("foo-123"), s!("foo-123"));
        assert_eq!(docker_tag_name("foo-123-"), s!("foo-123"));
    }
}
