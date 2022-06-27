use std::{env, path::PathBuf};

use crate::cargo::Subcommand;
use crate::config::bool_from_envvar;
use crate::errors::Result;
use crate::rustc::TargetList;
use crate::shell::{self, MessageInfo};
use crate::Target;

#[derive(Debug)]
pub struct Args {
    pub all: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub channel: Option<String>,
    pub target: Option<Target>,
    pub features: Vec<String>,
    pub target_dir: Option<PathBuf>,
    pub docker_in_docker: bool,
    pub enable_doctests: bool,
    pub manifest_path: Option<PathBuf>,
    pub version: bool,
    pub msg_info: MessageInfo,
}

// Fix for issue #581. target_dir must be absolute.
fn absolute_path(path: PathBuf) -> Result<PathBuf> {
    Ok(if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    })
}

pub fn is_subcommand_list(stdout: &str) -> bool {
    stdout.starts_with("Installed Commands:")
}

pub fn group_subcommands(stdout: &str) -> (Vec<&str>, Vec<&str>) {
    let mut cross = vec![];
    let mut host = vec![];
    for line in stdout.lines().skip(1) {
        // trim all whitespace, then grab the command name
        let first = line.split_whitespace().next();
        if let Some(command) = first {
            match Subcommand::from(command) {
                Subcommand::Other => host.push(line),
                _ => cross.push(line),
            }
        }
    }

    (cross, host)
}

pub fn fmt_subcommands(stdout: &str, msg_info: MessageInfo) -> Result<()> {
    let (cross, host) = group_subcommands(stdout);
    if !cross.is_empty() {
        shell::print("Cross Commands:", msg_info)?;
        for line in cross.iter() {
            shell::print(line, msg_info)?;
        }
    }
    if !host.is_empty() {
        shell::print("Host Commands:", msg_info)?;
        for line in cross.iter() {
            shell::print(line, msg_info)?;
        }
    }
    Ok(())
}

pub fn parse(target_list: &TargetList) -> Result<Args> {
    let mut channel = None;
    let mut target = None;
    let mut features = Vec::new();
    let mut manifest_path: Option<PathBuf> = None;
    let mut target_dir = None;
    let mut sc = None;
    let mut all: Vec<String> = Vec::new();
    let mut version = false;
    let mut quiet = false;
    let mut verbose = false;
    let mut color = None;
    let default_msg_info = MessageInfo::default();

    {
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg.is_empty() {
                continue;
            }
            if matches!(arg.as_str(), "--verbose" | "-v" | "-vv") {
                verbose = true;
            } else if matches!(arg.as_str(), "--version" | "-V") {
                version = true;
            } else if matches!(arg.as_str(), "--quiet" | "-q") {
                quiet = true;
            } else if arg == "--color" {
                match args.next() {
                    Some(arg) => color = Some(arg),
                    None => {
                        shell::fatal_usage("--color <WHEN>", default_msg_info, 1);
                    }
                }
            } else if arg == "--manifest-path" {
                all.push(arg);
                if let Some(m) = args.next() {
                    let p = PathBuf::from(&m);
                    all.push(m);
                    manifest_path = env::current_dir().ok().map(|cwd| cwd.join(p));
                }
            } else if arg.starts_with("--manifest-path=") {
                manifest_path = arg
                    .split_once('=')
                    .map(|x| x.1)
                    .map(PathBuf::from)
                    .and_then(|p| env::current_dir().ok().map(|cwd| cwd.join(p)));
                all.push(arg);
            } else if let ("+", ch) = arg.split_at(1) {
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
            } else if arg == "--features" {
                all.push(arg);
                if let Some(t) = args.next() {
                    features.push(t.clone());
                    all.push(t);
                }
            } else if arg.starts_with("--features=") {
                features.extend(arg.split_once('=').map(|(_, t)| t.to_owned()));
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
                if (!arg.starts_with('-') || arg == "--list") && sc.is_none() {
                    sc = Some(Subcommand::from(arg.as_ref()));
                }

                all.push(arg.to_string());
            }
        }
    }

    let msg_info = shell::MessageInfo::create(verbose, quiet, color.as_deref())?;
    let docker_in_docker = if let Ok(value) = env::var("CROSS_CONTAINER_IN_CONTAINER") {
        if env::var("CROSS_DOCKER_IN_DOCKER").is_ok() {
            shell::warn(
                "using both `CROSS_CONTAINER_IN_CONTAINER` and `CROSS_DOCKER_IN_DOCKER`.",
                msg_info,
            )?;
        }
        bool_from_envvar(&value)
    } else if let Ok(value) = env::var("CROSS_DOCKER_IN_DOCKER") {
        // FIXME: remove this when we deprecate CROSS_DOCKER_IN_DOCKER.
        bool_from_envvar(&value)
    } else {
        false
    };

    let enable_doctests = env::var("CROSS_UNSTABLE_ENABLE_DOCTESTS")
        .map(|s| bool_from_envvar(&s))
        .unwrap_or_default();

    Ok(Args {
        all,
        subcommand: sc,
        channel,
        target,
        features,
        target_dir,
        docker_in_docker,
        enable_doctests,
        manifest_path,
        version,
        msg_info,
    })
}
