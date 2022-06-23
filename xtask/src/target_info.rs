use std::{collections::BTreeMap, process::Stdio};

use crate::util::{format_repo, pull_image};
use clap::Args;
use cross::{docker, CommandExt};

// Store raw text data in the binary so we don't need a data directory
// when extracting all targets, or running our target info script.
const TARGET_INFO_SCRIPT: &str = include_str!("target_info.sh");

#[derive(Args, Debug)]
pub struct TargetInfo {
    /// If not provided, get info for all targets.
    targets: Vec<crate::ImageTarget>,
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Image registry.
    #[clap(long, default_value_t = String::from("ghcr.io"))]
    registry: String,
    /// Image repository.
    #[clap(long, default_value_t = String::from("cross-rs"))]
    repository: String,
    /// Image tag.
    #[clap(long, default_value_t = String::from("main"))]
    tag: String,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

fn image_info(
    engine: &docker::Engine,
    target: &crate::ImageTarget,
    image: &str,
    tag: &str,
    verbose: bool,
    has_test: bool,
) -> cross::Result<()> {
    if !tag.starts_with("local") {
        pull_image(engine, image, verbose)?;
    }

    let mut command = docker::command(engine);
    command.arg("run");
    command.arg("--rm");
    command.args(&["-e", &format!("TARGET={}", target.triplet)]);
    if has_test {
        command.args(&["-e", "HAS_TEST=1"]);
    } else {
        command.args(&["-e", "HAS_TEST="]);
    }
    command.arg(image);
    command.args(&["bash", "-c", TARGET_INFO_SCRIPT]);

    if !verbose {
        // capture stderr to avoid polluting table
        command.stderr(Stdio::null());
    }
    command.run(verbose, false).map_err(Into::into)
}

pub fn target_info(
    TargetInfo {
        mut targets,
        verbose,
        registry,
        repository,
        tag,
        ..
    }: TargetInfo,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let matrix = crate::util::get_matrix();
    let test_map: BTreeMap<crate::ImageTarget, bool> = matrix
        .iter()
        .map(|i| (i.to_image_target(), i.has_test(&i.target)))
        .collect();

    if targets.is_empty() {
        targets = matrix
            .iter()
            .map(|t| t.to_image_target())
            .filter(|t| t.has_ci_image())
            .collect();
    }

    for target in targets {
        let image = target.image_name(&format_repo(&registry, &repository), &tag);
        let has_test = test_map
            .get(&target)
            .cloned()
            .ok_or_else(|| eyre::eyre!("invalid target name {}", target))?;
        image_info(engine, &target, &image, &tag, verbose, has_test)?;
    }

    Ok(())
}
