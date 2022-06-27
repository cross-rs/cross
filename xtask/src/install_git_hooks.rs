use crate::util::project_dir;
use clap::Args;
use cross::shell::MessageInfo;

#[derive(Args, Debug)]
pub struct InstallGitHooks {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
}

pub fn install_git_hooks(
    InstallGitHooks {
        verbose,
        quiet,
        color,
    }: InstallGitHooks,
) -> cross::Result<()> {
    let msg_info = MessageInfo::create(verbose, quiet, color.as_deref())?;
    let root = project_dir(msg_info)?;
    let git_hooks = root.join(".git").join("hooks");
    let cross_dev = root.join("xtask").join("src");

    std::fs::copy(
        cross_dev.join("pre-commit.sh"),
        git_hooks.join("pre-commit"),
    )?;
    std::fs::copy(cross_dev.join("pre-push.sh"), git_hooks.join("pre-push"))?;

    Ok(())
}
