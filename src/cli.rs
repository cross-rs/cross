use std::{env, path::PathBuf};

use crate::cargo::Subcommand;
use crate::errors::Result;
use crate::rustc::TargetList;
use crate::shell::MessageInfo;
use crate::Target;

#[derive(Debug)]
pub struct Args {
    pub all: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub channel: Option<String>,
    pub target: Option<Target>,
    pub features: Vec<String>,
    pub target_dir: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub version: bool,
    pub verbose: bool,
    pub quiet: bool,
    pub color: Option<String>,
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

pub fn fmt_subcommands(stdout: &str, msg_info: &mut MessageInfo) -> Result<()> {
    let (cross, host) = group_subcommands(stdout);
    if !cross.is_empty() {
        msg_info.print("Cross Commands:")?;
        for line in &cross {
            msg_info.print(line)?;
        }
    }
    if !host.is_empty() {
        msg_info.print("Host Commands:")?;
        for line in &cross {
            msg_info.print(line)?;
        }
    }
    Ok(())
}

fn is_verbose(arg: &str) -> bool {
    match arg {
        "--verbose" => true,
        // cargo can handle any number of "v"s
        a => a
            .get(1..)
            .map(|a| a.chars().all(|x| x == 'v'))
            .unwrap_or_default(),
    }
}

enum ArgKind {
    Next,
    Equal,
}

fn is_value_arg(arg: &str, field: &str) -> Option<ArgKind> {
    if arg == field {
        Some(ArgKind::Next)
    } else if arg
        .strip_prefix(field)
        .map(|a| a.starts_with('='))
        .unwrap_or_default()
    {
        Some(ArgKind::Equal)
    } else {
        None
    }
}

fn parse_next_arg<T>(
    arg: String,
    out: &mut Vec<String>,
    parse: impl Fn(&str) -> T,
    iter: &mut impl Iterator<Item = String>,
) -> Option<T> {
    out.push(arg);
    match iter.next() {
        Some(next) => {
            let result = parse(&next);
            out.push(next);
            Some(result)
        }
        None => None,
    }
}

fn parse_equal_arg<T>(arg: String, out: &mut Vec<String>, parse: impl Fn(&str) -> T) -> T {
    let result = parse(arg.split_once('=').expect("argument should contain `=`").1);
    out.push(arg);

    result
}

fn parse_manifest_path(path: &str) -> Option<PathBuf> {
    let p = PathBuf::from(path);
    env::current_dir().ok().map(|cwd| cwd.join(p))
}

fn parse_target_dir(path: &str) -> Result<PathBuf> {
    absolute_path(PathBuf::from(path))
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
    let mut default_msg_info = MessageInfo::default();

    {
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg.is_empty() {
                continue;
            }
            if is_verbose(arg.as_str()) {
                verbose = true;
                all.push(arg);
            } else if matches!(arg.as_str(), "--version" | "-V") {
                version = true;
            } else if matches!(arg.as_str(), "--quiet" | "-q") {
                quiet = true;
                all.push(arg);
            } else if let Some(kind) = is_value_arg(&arg, "--color") {
                color = match kind {
                    ArgKind::Next => {
                        match parse_next_arg(arg, &mut all, ToOwned::to_owned, &mut args) {
                            Some(c) => Some(c),
                            None => default_msg_info.fatal_usage("--color <WHEN>", 1),
                        }
                    }
                    ArgKind::Equal => Some(parse_equal_arg(arg, &mut all, ToOwned::to_owned)),
                };
            } else if let Some(kind) = is_value_arg(&arg, "--manifest-path") {
                manifest_path = match kind {
                    ArgKind::Next => {
                        parse_next_arg(arg, &mut all, parse_manifest_path, &mut args).flatten()
                    }
                    ArgKind::Equal => parse_equal_arg(arg, &mut all, parse_manifest_path),
                };
            } else if let ("+", ch) = arg.split_at(1) {
                channel = Some(ch.to_owned());
            } else if let Some(kind) = is_value_arg(&arg, "--target") {
                target = match kind {
                    ArgKind::Next => {
                        parse_next_arg(arg, &mut all, |t| Target::from(t, target_list), &mut args)
                    }
                    ArgKind::Equal => Some(parse_equal_arg(arg, &mut all, |t| {
                        Target::from(t, target_list)
                    })),
                };
            } else if let Some(kind) = is_value_arg(&arg, "--features") {
                match kind {
                    ArgKind::Next => {
                        let next = parse_next_arg(arg, &mut all, ToOwned::to_owned, &mut args);
                        if let Some(feature) = next {
                            features.push(feature);
                        }
                    }
                    ArgKind::Equal => {
                        features.push(parse_equal_arg(arg, &mut all, ToOwned::to_owned));
                    }
                }
            } else if let Some(kind) = is_value_arg(&arg, "--target-dir") {
                match kind {
                    ArgKind::Next => {
                        all.push(arg);
                        if let Some(td) = args.next() {
                            target_dir = Some(parse_target_dir(&td)?);
                            all.push("/target".to_owned());
                        }
                    }
                    ArgKind::Equal => {
                        target_dir = Some(parse_target_dir(
                            arg.split_once('=').expect("argument should contain `=`").1,
                        )?);
                        all.push("--target-dir=/target".into());
                    }
                }
            } else {
                if (!arg.starts_with('-') || arg == "--list") && sc.is_none() {
                    sc = Some(Subcommand::from(arg.as_ref()));
                }

                all.push(arg.clone());
            }
        }
    }

    Ok(Args {
        all,
        subcommand: sc,
        channel,
        target,
        features,
        target_dir,
        manifest_path,
        version,
        verbose,
        quiet,
        color,
    })
}
