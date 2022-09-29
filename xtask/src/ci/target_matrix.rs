use clap::builder::BoolishValueParser;
use clap::Parser;
use serde::Serialize;

use crate::util::{get_matrix, gha_output, gha_print, CiTarget, ImageTarget};

pub(crate) fn run(message: String, author: String) -> Result<(), color_eyre::Report> {
    let mut matrix: Vec<CiTarget> = get_matrix().clone();
    if author == "bors[bot]" && message.starts_with("Try #") {
        if let Some((_, args)) = message.split_once(": ") {
            let app = TargetMatrixArgs::parse_from(args.split(' '));
            app.filter(&mut matrix);
        }
    } else {
        gha_print("Running all targets.");
    }

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
    gha_output("matrix", &serde_json::to_string(&matrix)?);
    Ok(())
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

#[derive(Parser, Debug)]
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
}

impl TargetMatrixArgs {
    pub fn filter(&self, matrix: &mut Vec<CiTarget>) {
        if self.none {
            std::mem::take(matrix);
            return;
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
}
