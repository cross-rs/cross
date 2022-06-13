use std::{path::Path, process::Command};

use clap::Args;
use cross::CommandExt;

#[derive(Args, Debug)]
pub struct BuildDockerImage {
    #[clap(long, hide = true, env = "GITHUB_REF_TYPE")]
    ref_type: Option<String>,
    #[clap(long, hide = true, env = "GITHUB_REF_NAME")]
    ref_name: Option<String>,
    /// Newline separated labels
    #[clap(long, env = "LABELS")]
    labels: Option<String>,
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    verbose: bool,
    #[clap(long)]
    force: bool,
    #[clap(short, long)]
    push: bool,
    #[clap(long, possible_values = ["auto", "plain", "tty"], default_value = "auto")]
    progress: String,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,

    /// Targets to build for
    #[clap()]
    targets: Vec<String>,
}

pub fn build_docker_image(
    BuildDockerImage {
        mut targets,
        verbose,
        force,
        push,
        progress,
        ref_type,
        ref_name,
        labels,
        ..
    }: BuildDockerImage,
    engine: &Path,
) -> cross::Result<()> {
    let metadata = cross::cargo_metadata_with_args(
        Some(Path::new(env!("CARGO_MANIFEST_DIR"))),
        None,
        verbose,
    )?
    .ok_or_else(|| eyre::eyre!("could not find cross workspace and its current version"))?;
    let version = metadata
        .get_package("cross")
        .expect("cross expected in workspace")
        .version
        .clone();
    if targets.is_empty() {
        targets = walkdir::WalkDir::new(metadata.workspace_root.join("docker"))
            .max_depth(1)
            .contents_first(true)
            .into_iter()
            .filter_map(|e| e.ok().filter(|f| f.file_type().is_file()))
            .filter_map(|f| {
                f.file_name()
                    .to_string_lossy()
                    .strip_prefix("Dockerfile.")
                    .map(ToOwned::to_owned)
            })
            .collect();
    }
    for target in targets {
        let mut docker_build = Command::new(engine);
        docker_build.args(&["buildx", "build"]);
        docker_build.current_dir(metadata.workspace_root.join("docker"));

        if push {
            docker_build.arg("--push");
        } else {
            docker_build.arg("--load");
        }

        let dockerfile = format!("Dockerfile.{target}");
        let image_name = format!("{}/{target}", cross::CROSS_IMAGE);
        let mut tags = vec![];

        match (ref_type.as_deref(), ref_name.as_deref()) {
            (Some(ref_type), Some(ref_name)) if ref_type == "tag" && ref_name.starts_with('v') => {
                let tag_version = ref_name
                    .strip_prefix('v')
                    .expect("tag name should start with v");
                if version != tag_version {
                    eyre::bail!("git tag does not match package version.")
                }
                tags.push(format!("{image_name}:{version}"));
                // Check for unstable releases, tag stable releases as `latest`
                if version.contains('-') {
                    // TODO: Don't tag if version is older than currently released version.
                    tags.push(format!("{image_name}:latest"))
                }
            }
            (Some(ref_type), Some(ref_name)) if ref_type == "branch" => {
                tags.push(format!("{image_name}:{ref_name}"));

                if ["staging", "trying"]
                    .iter()
                    .any(|branch| branch == &ref_name)
                {
                    tags.push(format!("{image_name}:edge"));
                }
            }
            _ => {
                if push {
                    panic!("Refusing to push without tag or branch. {ref_type:?}:{ref_name:?}")
                }
                tags.push(format!("{image_name}:local"))
            }
        }

        docker_build.arg("--pull");
        docker_build.args(&[
            "--cache-from",
            &format!("type=registry,ref={image_name}:main"),
        ]);

        if push {
            docker_build.args(&["--cache-to", "type=inline"]);
        }

        for tag in &tags {
            docker_build.args(&["--tag", tag]);
        }

        for label in labels
            .as_deref()
            .unwrap_or("")
            .split('\n')
            .filter(|s| !s.is_empty())
        {
            docker_build.args(&["--label", label]);
        }

        docker_build.args(&["-f", &dockerfile]);

        if std::env::var("GITHUB_ACTIONS").is_ok() || progress == "plain" {
            docker_build.args(&["--progress", "plain"]);
        } else {
            docker_build.args(&["--progress", &progress]);
        }

        docker_build.arg(".");

        if force || !push || std::env::var("GITHUB_ACTIONS").is_ok() {
            docker_build.run(verbose)?;
        } else {
            docker_build.print_verbose(true);
        }
        if std::env::var("GITHUB_ACTIONS").is_ok() {
            println!("::set-output name=image::{}", &tags[0])
        }
    }
    if (std::env::var("GITHUB_ACTIONS").is_ok() || !force) && push {
        panic!("refusing to push, use --force to override");
    }
    Ok(())
}
