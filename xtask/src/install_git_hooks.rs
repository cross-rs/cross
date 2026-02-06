use std::{path::PathBuf, process::Command};

use crate::util::project_dir;
use clap::Args;
use cross::{shell::MessageInfo, CommandExt};

#[derive(Args, Debug)]
pub struct InstallGitHooks {}

pub fn install_git_hooks(msg_info: &mut MessageInfo) -> cross::Result<()> {
    let root = project_dir(msg_info)?;
    let git_hooks = Command::new("git")
        .args(&["rev-parse", "--git-common-dir"])
        .run_and_get_stdout(msg_info)
        .map(|s| PathBuf::from(&s.trim()))?
        .join("hooks");
    let cross_dev = root.join("xtask").join("src");

    std::fs::copy(
        cross_dev.join("pre-commit.sh"),
        git_hooks.join("pre-commit"),
    )?;
    std::fs::copy(cross_dev.join("pre-push.sh"), git_hooks.join("pre-push"))?;

    Ok(())
}
