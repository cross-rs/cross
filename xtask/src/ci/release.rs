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
