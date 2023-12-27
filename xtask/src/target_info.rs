use std::collections::BTreeMap;

use crate::util::{format_repo, pull_image};
use clap::Args;
use cross::shell::MessageInfo;
use cross::{docker, CommandExt};

// Store raw text data in the binary so we don't need a data directory
// when extracting all targets, or running our target info script.
const TARGET_INFO_SCRIPT: &str = include_str!("target_info.sh");

#[derive(Args, Debug)]
pub struct TargetInfo {
    /// If not provided, get info for all targets.
    pub targets: Vec<crate::ImageTarget>,
    /// Image registry.
    #[clap(long, default_value_t = String::from("ghcr.io"))]
    pub registry: String,
    /// Image repository.
    #[clap(long, default_value_t = String::from("cross-rs"))]
    pub repository: String,
    /// Image tag.
    #[clap(long, default_value_t = String::from("main"))]
    pub tag: String,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

fn image_info(
    engine: &docker::Engine,
    target: &crate::ImageTarget,
    image: &str,
    tag: &str,
    msg_info: &mut MessageInfo,
    has_test: bool,
) -> cross::Result<()> {
    if !tag.starts_with("local") {
        pull_image(engine, image, msg_info)?;
    }

    let mut command = engine.command();
    command.arg("run");
    command.arg("--rm");
    command.args(["-e", &format!("TARGET={}", target.name)]);
    if msg_info.is_verbose() {
        command.args(["-e", "VERBOSE=1"]);
    }
    if has_test {
        command.args(["-e", "HAS_TEST=1"]);
    } else {
        command.args(["-e", "HAS_TEST="]);
    }
    command.arg(image);
    command.args(["bash", "-c", TARGET_INFO_SCRIPT]);
    command.run(msg_info, msg_info.is_verbose())
}

pub fn target_info(
    TargetInfo {
        mut targets,
        registry,
        repository,
        tag,
        ..
    }: TargetInfo,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
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
        image_info(engine, &target, &image, &tag, msg_info, has_test)?;
    }

    Ok(())
}
