use crate::util::project_dir;
use clap::Args;

#[derive(Args, Debug)]
pub struct InstallGitHooks {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    verbose: bool,
}

pub fn install_git_hooks(InstallGitHooks { verbose }: InstallGitHooks) -> cross::Result<()> {
    let root = project_dir(verbose)?;
    let git_hooks = root.join(".git").join("hooks");
    let cross_dev = root.join("xtask").join("src");
    std::fs::copy(
        cross_dev.join("pre-commit.sh"),
        git_hooks.join("pre-commit"),
    )?;
    std::fs::copy(cross_dev.join("pre-push.sh"), git_hooks.join("pre-push"))?;

    Ok(())
}
