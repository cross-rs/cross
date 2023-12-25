// TODO: More details/ensure accuracy
//!
//! # The `publish` job + `build`
//!
//! The `publish` job is triggered in five cases.
//! All of these need the event to be a push (since the `build` job is a need for `publish`,
//! and that need is gated on `if: github.event_name == 'push'`
//!
//! 1. on the default_branch, or `main` in our case
//! 2. on the `staging` branch
//! 3. on the `try` branch
//! 4. on branches matching `v*.*.*`
//! 5. on tags matching `v*.*.*`
//!
//! ## In `default_branch`/`main`
//!
//! In the case of `main`, the workflow does the following.
//!
//! 1. `build` builds and publishes images for these targets with the tag `main` and `edge`. It also assembles binaries for artifacting
//! 2. If all ok, the `publish` job triggers
//! 3. this calls `cargo xtask ci-job release` which
//!    1. inspects the package version
//!    2. if the version does not exist as a tag, create a new tag for that version and push it.
//!       this tag will trigger the `CI` workflow again, but with `ref_type == "tag"`
//!       if the version does exist, exit quietly.
//! 4. `publish` now calls `cargo-publish` which creates a new release with draft tag `Unreleased`, attaches binaries from step 1, and does a `cargo publish --dry-run`, this tag uses the standard github token for workflows, and should not be able to trigger any other workflows.
//!
//! ## In `staging` / `try` branch
//!
//! In `staging` or `try`, we need to make sure that nothing goes out.
//! This includes tags, releases and `cargo publish`
//!
//! 1. `build` builds (but does not publish) images for these targets with the tag `try`/`staging`. It also assembles binaries for artifacting
//! 2. If all ok, the `publish` job triggers
//! 4. this calls `cargo xtask ci-job release` which
//!    1. inspects the package version
//!    2. if the version does not exist as a tag, "dry-run" creating the tag and push it.
//!       if the version does exist, exit quietly.
//! 5. `publish` now calls `cargo-publish` which does a `cargo publish --dry-run`
//!
//! ## On branches matching `v*.*.*`
//!
//! 1. `build` builds (but does not publish) images for these targets with the tag `vx.y.z` and `edge`. It also assembles binaries for artifacting
//! 2. If all ok, the `publish` job triggers
//! 3. this calls `cargo xtask ci-job release` which
//!    1. inspects the package version
//!    2. since the `ref_type == "branch"`, if the version does not exist as a tag,
//!       create a new tag for that version and push it.
//!       this tag will trigger the `CI` workflow again, but with `ref_type == "tag"`
//!       if the version does exist, exit quietly.
//! 4. `publish` now calls `cargo-publish` which does nothing
//!
//! ## On tags matching `v*.*.*`
//!
//! In this case, we need to make sure that the created release does not trigger a workflow.
//!
//! 1. `build` builds and publishes images for these targets with the tag `vx.y.z`. It also assembles binaries for artifacting
//! 2. If all ok, the `publish` job triggers
//! 4. this calls `cargo xtask ci-job release` which
//!    1. inspects the package version
//!    2. since the `ref_type == "tag"`, the program exits quietly.
//! 5. `publish` now calls `cargo-publish` which creates a new release with tag `vx.y.z`, attaches binaries from step 1, and does a `cargo publish`, this release tag uses the standard github token for workflows, and should not be able to trigger any other workflows.
use clap::Args;
use cross::{shell::MessageInfo, CommandExt};

#[derive(Debug, Args)]
pub struct Release {
    #[clap(long, default_value = "main", env = "DEFAULT_BRANCH")]
    default_branch: String,
    #[clap(long, hide = true, env = "GITHUB_REF_TYPE")]
    pub ref_type: Option<String>,
    #[clap(long, hide = true, env = "GITHUB_REF_NAME")]
    ref_name: Option<String>,
}

impl Release {
    pub fn run(&self, msg_info: &mut MessageInfo) -> Result<(), color_eyre::Report> {
        if self.ref_type.as_deref() == Some("branch") {
            self.tag(msg_info)?;
        }
        Ok(())
    }

    pub fn tag(&self, msg_info: &mut MessageInfo) -> Result<(), color_eyre::Report> {
        color_eyre::eyre::ensure!(
            self.ref_type.as_deref() == Some("branch"),
            "tag() should only be called on a branch"
        );
        let current_branch = self.ref_name.as_deref().unwrap();
        let version = pkgid()?.rsplit_once('#').unwrap().1.trim().to_string();
        let tag = format!("v{version}");

        let has_tag = std::process::Command::new("git")
            .args(["tag", "--list"])
            .run_and_get_stdout(msg_info)?
            .lines()
            .any(|it| it.trim() == tag);
        if !has_tag {
            let dry_run = std::env::var("CI").is_err()
                || (current_branch != self.default_branch
                    && !wildmatch::WildMatch::new("v*.*.*").matches(current_branch));

            eprint!("Taging!");
            let mut tagging = std::process::Command::new("git");
            tagging.args(["tag", &tag]);
            let mut push = std::process::Command::new("git");
            push.args(["push", "--tags"]);
            if dry_run {
                eprintln!(" (dry run)");
                tagging.print(msg_info)?;
                push.print(msg_info)?;
            } else {
                eprintln!();
                tagging.run(msg_info, false)?;
                push.run(msg_info, false)?;
            }
        }
        Ok(())
    }
}

#[track_caller]
fn pkgid() -> Result<String, color_eyre::Report> {
    cross::cargo_command()
        .arg("pkgid")
        .current_dir(crate::util::get_cargo_workspace())
        .run_and_get_stdout(&mut cross::shell::Verbosity::Verbose(1).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_pkgid_hashtag() {
        let pkgid = pkgid().unwrap();
        assert!(!pkgid.contains('@'));
        assert!(pkgid.contains("cross"));
    }
}
