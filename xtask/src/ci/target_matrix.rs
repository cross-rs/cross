use std::process::Command;

use clap::builder::BoolishValueParser;
use clap::Parser;
use cross::{shell::Verbosity, CommandExt};
use serde::{Deserialize, Serialize};

use crate::util::{get_matrix, gha_output, gha_print, CiTarget, ImageTarget};

pub(crate) fn run(message: String, author: String, weekly: bool) -> Result<(), color_eyre::Report> {
    let mut matrix: Vec<CiTarget> = get_matrix().clone();
    let (prs, mut app) = if author == "bors[bot]" {
        process_bors_message(&message)?
    } else if weekly {
        let app = TargetMatrixArgs {
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
        };
        (vec![], app)
    } else {
        (vec![], TargetMatrixArgs::default())
    };

    if !prs.is_empty()
        && prs.iter().try_fold(true, |b, pr| {
            Ok::<_, eyre::Report>(b && has_no_ci_target(pr)?)
        })?
    {
        app.none = true;
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
        })
        .collect::<Vec<_>>();

    let json = serde_json::to_string(&matrix)?;
    gha_print(&json);
    gha_output("matrix", &json)?;
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

fn has_no_ci_target(pr: &str) -> cross::Result<bool> {
    Ok(parse_gh_labels(pr)?.contains(&"no-ci-targets".to_owned()))
}

/// Returns the pr(s) associated with this bors commit and the app to use for processing
fn process_bors_message(message: &str) -> cross::Result<(Vec<&str>, TargetMatrixArgs)> {
    if let Some(message) = message.strip_prefix("Try #") {
        let (pr, args) = message
            .split_once(':')
            .ok_or_else(|| eyre::eyre!("bors message must start with \"Try #:\""))?;
        let args = args.trim_start();
        let app = if !args.is_empty() {
            TargetMatrixArgs::parse_from(args.split(' '))
        } else {
            TargetMatrixArgs::default()
        };
        Ok((vec![pr], app))
    } else if let Some(message) = message.strip_prefix("Merge") {
        Ok((
            message
                .lines()
                .next()
                .unwrap_or_default()
                .split(" #")
                .skip(1)
                .collect(),
            TargetMatrixArgs::default(),
        ))
    } else {
        eyre::bail!("unexpected bors commit message encountered")
    }
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
}

#[derive(Parser, Debug, Default, PartialEq, Eq)]
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[track_caller]
    fn run<'a>(args: impl IntoIterator<Item = &'a str>) -> Vec<CiTarget> {
        let mut matrix = get_matrix().clone();
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
        assert_eq!(get_matrix(), &matrix);
    }

    #[test]
    fn none() {
        let matrix = run(["--none"]);
        assert_eq!(&Vec::<CiTarget>::new(), &matrix);
    }

    #[test]
    fn prs() {
        assert_eq!(
            process_bors_message("Merge #1337\n1337: merge").unwrap().0,
            vec!["1337"]
        );
        assert_eq!(
            process_bors_message("Merge #1337 #42\n1337: merge\n42: merge 2")
                .unwrap()
                .0,
            vec!["1337", "42"]
        );
        assert_eq!(
            // the trailing space is intentional
            process_bors_message("Try #1337: \n").unwrap().0,
            vec!["1337"]
        );
    }

    #[test]
    fn full_invocation() {
        let (prs, app) = process_bors_message("Try #1337: ").unwrap();
        assert_eq!(prs, vec!["1337"]);
        assert_eq!(app, TargetMatrixArgs::default());
        let (prs, app) = process_bors_message("Try #1337: --std 1").unwrap();
        assert_eq!(prs, vec!["1337"]);
        assert_eq!(
            app,
            TargetMatrixArgs {
                std: Some(true),
                ..TargetMatrixArgs::default()
            }
        );
    }
}
