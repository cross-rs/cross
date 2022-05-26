use std::str::FromStr;
use std::{env, path::PathBuf};

use crate::cargo::Subcommand;
use crate::errors::Result;
use crate::rustc::TargetList;
use crate::Target;

#[derive(Debug)]
pub struct Args {
    pub all: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub channel: Option<String>,
    pub target: Option<Target>,
    pub target_dir: Option<PathBuf>,
    pub docker_in_docker: bool,
    pub enable_doctests: bool,
}

// Fix for issue #581. target_dir must be absolute.
fn absolute_path(path: PathBuf) -> Result<PathBuf> {
    Ok(if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    })
}

fn bool_from_envvar(envvar: &str) -> bool {
    if let Ok(value) = bool::from_str(envvar) {
        value
    } else if let Ok(value) = i32::from_str(envvar) {
        value != 0
    } else {
        !envvar.is_empty()
    }
}

pub fn parse(target_list: &TargetList) -> Result<Args> {
    let mut channel = None;
    let mut target = None;
    let mut target_dir = None;
    let mut sc = None;
    let mut all: Vec<String> = Vec::new();

    {
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg.is_empty() {
                continue;
            }
            if let ("+", ch) = arg.split_at(1) {
                channel = Some(ch.to_string());
            } else if arg == "--target" {
                all.push(arg);
                if let Some(t) = args.next() {
                    target = Some(Target::from(&t, target_list));
                    all.push(t);
                }
            } else if arg.starts_with("--target=") {
                target = arg
                    .split_once('=')
                    .map(|(_, t)| Target::from(t, target_list));
                all.push(arg);
            } else if arg == "--target-dir" {
                all.push(arg);
                if let Some(td) = args.next() {
                    target_dir = Some(absolute_path(PathBuf::from(&td))?);
                    all.push("/target".to_string());
                }
            } else if arg.starts_with("--target-dir=") {
                if let Some((_, td)) = arg.split_once('=') {
                    target_dir = Some(absolute_path(PathBuf::from(&td))?);
                    all.push("--target-dir=/target".into());
                }
            } else {
                if !arg.starts_with('-') && sc.is_none() {
                    sc = Some(Subcommand::from(arg.as_ref()));
                }

                all.push(arg.to_string());
            }
        }
    }

    let docker_in_docker = env::var("CROSS_DOCKER_IN_DOCKER")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or_default();
    let enable_doctests = env::var("CROSS_UNSTABLE_ENABLE_DOCTESTS")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or_default();

    Ok(Args {
        all,
        subcommand: sc,
        channel,
        target,
        target_dir,
        docker_in_docker,
        enable_doctests,
    })
}
