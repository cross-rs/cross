use clap::Args;

use std::path::Path;

#[derive(Args, Debug)]
pub struct InstallGitHooks {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    verbose: bool,
}

pub fn install_git_hooks(InstallGitHooks { verbose }: InstallGitHooks) -> cross::Result<()> {
    let metadata = cross::cargo_metadata_with_args(
        Some(Path::new(env!("CARGO_MANIFEST_DIR"))),
        None,
        verbose,
    )?
    .ok_or_else(|| eyre::eyre!("could not find cross workspace"))?;
    let git_hooks = metadata.workspace_root.join(".git").join("hooks");
    let cross_dev = metadata.workspace_root.join("xtask").join("src");
    std::fs::copy(cross_dev.join("pre-commit"), git_hooks.join("pre-commit"))?;

    Ok(())
}
