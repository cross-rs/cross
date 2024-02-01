use std::process::Command;

use clap::builder::{BoolishValueParser, PossibleValuesParser};
use clap::Parser;
use cross::{shell::Verbosity, CommandExt};
use serde::{Deserialize, Serialize};

use crate::util::{get_matrix, gha_output, gha_print, CiTarget, ImageTarget};

#[derive(Parser, Debug)]
pub struct TargetMatrix {
    /// check is being run as part of a weekly check
    #[clap(long)]
    pub weekly: bool,
    /// merge group that is being checked.
    #[clap(long)]
    pub merge_group: Option<String>,
    #[clap(subcommand)]
    pub subcommand: Option<TargetMatrixSub>,
}

#[derive(Parser, Debug)]
pub enum TargetMatrixSub {
    Try {
        /// pr to check
        #[clap(long)]
        pr: String,
        /// comment to check
        #[clap(long)]
        comment: String,
    },
}

impl TargetMatrix {
    pub(crate) fn run(&self) -> Result<(), color_eyre::Report> {
        let mut matrix: Vec<CiTarget> = get_matrix().clone();
        matrix.retain(|t| !t.disabled);
        let mut is_default_try = false;
        let pr: Option<String>;
        let (prs, mut app) = match self {
            TargetMatrix {
                merge_group: Some(ref_),
                ..
            } => (
                vec![process_merge_group(ref_)?],
                TargetMatrixArgs::default(),
            ),
            TargetMatrix { weekly: true, .. } => (
                vec![],
                TargetMatrixArgs {
                    target: std::env::var("TARGETS")
                        .unwrap_or_default()
                        .split(' ')
                        .flat_map(|s| s.split(','))
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_owned())
                        .collect(),
                    std: None,
                    cpp: None,
                    dylib: None,
                    run: None,
                    runners: vec![],
                    none: false,
                    has_image: true,
                    verbose: false,
                    tests: vec!["all".to_owned()],
                },
            ),
            TargetMatrix {
                subcommand: Some(TargetMatrixSub::Try { pr, comment }),
                ..
            } => {
                let process_try_comment = process_try_comment(comment)?;
                is_default_try = process_try_comment.0;
                (vec![pr.as_ref()], process_try_comment.1)
            }
            _ => {
                pr = current_pr();
                (
                    pr.iter().map(|s| s.as_str()).collect(),
                    TargetMatrixArgs::default(),
                )
            }
        };

        // only apply ci labels on prs and `/ci try`,
        // if the try command is not the default, we don't want to apply ci labels
        if matches!(
            self,
            Self {
                weekly: false,
                merge_group: Some(_) | None,
                subcommand: None,
            }
        ) || is_default_try
        {
            apply_ci_labels(&prs, &mut app)?
        }

        app.filter(&mut matrix);

        let matrix = matrix
            .iter()
            .map(|target| TargetMatrixElement {
                pretty: target.to_image_target().alt(),
                platforms: target.platforms(),
                target: &target.target,
                sub: target.sub.as_deref(),
                os: &target.os,
                run: target.run.map(|b| b as u8),
                deploy: target.deploy.map(|b| b as u8),
                build_std: target.build_std.map(|b| b as u8),
                cpp: target.cpp.map(|b| b as u8),
                dylib: target.dylib.map(|b| b as u8),
                runners: target.runners.as_deref(),
                std: target.std.map(|b| b as u8),
                verbose: app.verbose,
            })
            .collect::<Vec<_>>();

        let json = serde_json::to_string(&matrix)?;
        gha_output("matrix", &json)?;
        let tests = serde_json::to_string(&app.tests()?)?;
        gha_output("tests", &tests)?;
        Ok(())
    }
}

fn apply_ci_labels(prs: &[&str], app: &mut TargetMatrixArgs) -> Result<(), eyre::Error> {
    apply_has_no_ci_tests(prs, app)?;
    apply_has_no_ci_target(prs, app)?;

    let mut to_add = vec![];
    'pr_loop: for pr in prs {
        let labels = parse_gh_labels(pr)?;
        let targets = labels
            .iter()
            .filter_map(|label| label.strip_prefix("CI-"))
            .collect::<Vec<_>>();
        if targets.is_empty() {
            // if a pr doesn't specify a target, assume it affects all targets
            to_add.clear();
            break 'pr_loop;
        }
        to_add.extend(targets.iter().map(|label| label.to_string()));
    }
    app.target.extend(to_add);
    Ok(())
}

fn apply_has_no_ci_tests(prs: &[&str], app: &mut TargetMatrixArgs) -> Result<(), eyre::Error> {
    if !prs.is_empty()
        && prs.iter().try_fold(true, |b, pr| {
            Ok::<_, eyre::Report>(b && has_no_ci_tests_label(pr)?)
        })?
    {
        app.none = true;
        app.tests = vec!["none".to_owned()];
    }
    Ok(())
}

fn apply_has_no_ci_target(prs: &[&str], app: &mut TargetMatrixArgs) -> Result<(), eyre::Error> {
    if !prs.is_empty()
        && prs.iter().try_fold(true, |b, pr| {
            Ok::<_, eyre::Report>(b && has_no_ci_target_label(pr)?)
        })?
    {
        app.none = true;
    }
    Ok(())
}

fn parse_gh_labels(pr: &str) -> cross::Result<Vec<String>> {
    #[derive(Deserialize)]
    struct PullRequest {
        labels: Vec<PullRequestLabels>,
    }

    #[derive(Deserialize)]
    struct PullRequestLabels {
        name: String,
    }
    eyre::ensure!(
        pr.chars().all(|c| c.is_ascii_digit()),
        "pr should be a number, got {:?}",
        pr
    );
    let stdout = Command::new("gh")
        .args(["pr", "view", pr, "--json", "labels"])
        .run_and_get_stdout(&mut Verbosity::Quiet.into())?;
    let pr_info: PullRequest = serde_json::from_str(&stdout)?;
    Ok(pr_info.labels.into_iter().map(|l| l.name).collect())
}

fn has_no_ci_target_label(pr: &str) -> cross::Result<bool> {
    Ok(parse_gh_labels(pr)?.contains(&"no-ci-targets".to_owned()))
}

fn has_no_ci_tests_label(pr: &str) -> cross::Result<bool> {
    Ok(parse_gh_labels(pr)?.contains(&"no-ci-tests".to_owned()))
}

/// Convert a `GITHUB_REF` into it's merge group pr
fn process_merge_group(ref_: &str) -> cross::Result<&str> {
    ref_.split('/')
        .last()
        .unwrap_or_default()
        .strip_prefix("pr-")
        .ok_or_else(|| eyre::eyre!("merge group ref must start last / segment with \"pr-\""))?
        .split('-')
        .next()
        .ok_or_else(|| eyre::eyre!("merge group ref must include \"pr-<num>-<sha>\""))
}

fn current_pr() -> Option<String> {
    // gh pr view --json number --template "{{.number}}"
    let stdout = Command::new("gh")
        .args(["pr", "view", "--json", "number"])
        .run_and_get_stdout(&mut Verbosity::Quiet.into())
        .ok()?;
    let pr_info: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    pr_info.get("number").map(|n| n.to_string())
}

/// Returns app to use for matrix on try comment, boolean is used to determine if its a try without arguments
fn process_try_comment(message: &str) -> cross::Result<(bool, TargetMatrixArgs)> {
    for line in message.lines() {
        let command = if let Some(command) = line.strip_prefix("/ci try") {
            command.trim()
        } else {
            continue;
        };
        if command.is_empty() {
            return Ok((true, TargetMatrixArgs::default()));
        } else {
            return Ok((false, TargetMatrixArgs::parse_from(command.split(' '))));
        }
    }
    eyre::bail!("no /ci try command found in comment")
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct TargetMatrixElement<'a> {
    pretty: String,
    platforms: &'a [String],
    target: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub: Option<&'a str>,
    os: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    run: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deploy: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    build_std: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpp: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dylib: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runners: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    std: Option<u8>,
    verbose: bool,
}

#[derive(Parser, Debug, PartialEq, Eq)]
#[clap(no_binary_name = true)]
struct TargetMatrixArgs {
    #[clap(long, short, num_args = 0..)]
    target: Vec<String>,
    #[clap(long, value_parser = BoolishValueParser::new())]
    std: Option<bool>,
    #[clap(long, value_parser = BoolishValueParser::new())]
    cpp: Option<bool>,
    #[clap(long, value_parser = BoolishValueParser::new())]
    dylib: Option<bool>,
    #[clap(long, value_parser = BoolishValueParser::new())]
    run: Option<bool>,
    #[clap(long, short, num_args = 0..)]
    runners: Vec<String>,
    #[clap(long)]
    none: bool,
    #[clap(long)]
    has_image: bool,
    #[clap(long, short)]
    verbose: bool,
    #[clap(long, value_parser = PossibleValuesParser::new(&[
            "remote",
            "bisect",
            "foreign",
            "docker-in-docker",
            "podman",
            "none",
            "all"
        ]),
        num_args = 0..,
        value_delimiter = ',',
        default_value = "all"
    )]
    tests: Vec<String>,
}

impl Default for TargetMatrixArgs {
    fn default() -> Self {
        Self {
            target: Vec::new(),
            std: None,
            cpp: None,
            dylib: None,
            run: None,
            runners: Vec::new(),
            none: false,
            has_image: false,
            verbose: false,
            tests: vec!["all".to_owned()],
        }
    }
}

impl TargetMatrixArgs {
    pub fn filter(&self, matrix: &mut Vec<CiTarget>) {
        if self == &TargetMatrixArgs::default() {
            gha_print("Running all targets.");
        }
        if self.none {
            gha_print("Running no targets.");
            std::mem::take(matrix);
            return;
        }
        if self.has_image {
            matrix.retain(|t| t.to_image_target().has_ci_image());
        }
        if !self.target.is_empty() {
            matrix.retain(|m| {
                let matrix_target = m.to_image_target();
                let matrix_string = matrix_target.to_string();

                return self
                    .target
                    .iter()
                    .any(|t| t.parse::<ImageTarget>().unwrap() == matrix_target)
                    || self
                        .target
                        .iter()
                        .any(|t| wildmatch::WildMatch::new(t).matches(&matrix_string));
            })
        };
        if let Some(std) = self.std {
            matrix.retain(|m| m.std.unwrap_or_default() == std)
        }
        if let Some(cpp) = self.cpp {
            matrix.retain(|m| m.cpp.unwrap_or_default() == cpp)
        }
        if let Some(dylib) = self.dylib {
            matrix.retain(|m| m.dylib.unwrap_or_default() == dylib)
        }
        if let Some(run) = self.run {
            matrix.retain(|m| m.run.unwrap_or_default() == run)
        }
        if !self.runners.is_empty() {
            matrix.retain(|m| {
                self.runners
                    .iter()
                    .any(|runner| m.runners.as_deref().unwrap_or_default().contains(runner))
            });
        }
    }

    fn tests(&self) -> Result<serde_json::Value, serde_json::Error> {
        use clap::CommandFactory;
        use serde::ser::SerializeMap;
        struct Ser(Vec<String>);
        impl serde::Serialize for Ser {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for e in &self.0 {
                    map.serialize_entry(&e, &true)?;
                }
                map.end()
            }
        }
        let mut tests = match (
            self.tests.iter().any(|t| t == "all"),
            self.tests.iter().any(|t| t == "none"),
        ) {
            (_, true) => vec![],
            (true, false) => {
                let possible: Vec<String> = Self::command()
                    .get_arguments()
                    .find(|arg| arg.get_id() == "tests")
                    .expect("a `tests` argument should exist")
                    .get_possible_values()
                    .into_iter()
                    .map(|p| p.get_name().to_owned())
                    .collect();

                possible
            }
            _ => self.tests.clone(),
        };
        tests.retain(|p| p != "all");
        serde_json::to_value(Ser(tests))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[track_caller]
    fn run<'a>(args: impl IntoIterator<Item = &'a str>) -> Vec<CiTarget> {
        let mut matrix = get_matrix().clone();
        matrix.retain_mut(|t| !t.disabled);
        TargetMatrixArgs::try_parse_from(args)
            .unwrap()
            .filter(&mut matrix);
        matrix
    }

    #[test]
    fn it_works() {
        run([
            "--target",
            "*",
            "--std",
            "1",
            "--cpp",
            "1",
            "--dylib",
            "1",
            "--run",
            "1",
            "--runners",
            "native",
        ]);
    }

    #[test]
    fn exact() {
        let matrix = run(["--target", "arm-unknown-linux-gnueabi"]);
        assert_eq!(matrix.len(), 1);
        assert_eq!(matrix[0].target, "arm-unknown-linux-gnueabi");
    }

    #[test]
    fn glob() {
        let matrix = run(["--target", "arm-unknown-linux-gnueabi*"]);
        assert_eq!(matrix.len(), 2);
        assert_eq!(matrix[0].target, "arm-unknown-linux-gnueabi");
        assert_eq!(matrix[1].target, "arm-unknown-linux-gnueabihf");
    }

    #[test]
    fn ensure_filter_works() {
        let matrix = run(["--dylib", "1"]);
        assert!(matrix
            .iter()
            .any(|t| t.target == "aarch64-unknown-linux-gnu"));
        assert!(matrix.iter().all(|t| t.target != "thumbv6m-none-eabi"));

        let matrix = run(["--dylib", "0"]);
        assert!(matrix
            .iter()
            .all(|t| t.target != "aarch64-unknown-linux-gnu"));
        assert!(matrix.iter().any(|t| t.target == "thumbv6m-none-eabi"));
    }

    #[test]
    fn all() {
        let matrix = run([]);
        let mut all = get_matrix().clone();
        all.retain(|t| !t.disabled);
        assert_eq!(&all, &matrix);
    }

    #[test]
    fn none() {
        let matrix = run(["--none"]);
        assert_eq!(&Vec::<CiTarget>::new(), &matrix);
    }

    #[test]
    fn merge_group() {
        assert_eq!(
            process_merge_group("refs/heads/gh-readonly-queue/main/pr-1375-44011c8854cb2eaac83b173cc323220ccdff18ea").unwrap(),
            "1375"
        );
    }
}
