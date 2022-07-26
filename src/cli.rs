use std::env;
use std::path::{Path, PathBuf};

use crate::cargo::Subcommand;
use crate::cargo_config::CargoConfig;
use crate::errors::Result;
use crate::file::{absolute_path, PathExt};
use crate::rustc::TargetList;
use crate::shell::{self, MessageInfo};
use crate::Target;

#[derive(Debug, Default)]
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
        a => {
            a.starts_with('-')
                && a.len() >= 2
                && a.get(1..)
                    .map(|a| a.chars().all(|x| x == 'v'))
                    .unwrap_or_default()
        }
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
    parse: impl Fn(&str) -> Result<T>,
    store_cb: impl Fn(String) -> Result<String>,
    iter: &mut impl Iterator<Item = String>,
) -> Result<Option<T>> {
    out.push(arg);
    match iter.next() {
        Some(next) => {
            let result = parse(&next)?;
            out.push(store_cb(next)?);
            Ok(Some(result))
        }
        None => Ok(None),
    }
}

fn parse_equal_arg<T>(
    arg: String,
    out: &mut Vec<String>,
    parse: impl Fn(&str) -> Result<T>,
    store_cb: impl Fn(String) -> Result<String>,
) -> Result<T> {
    let (first, second) = arg.split_once('=').expect("argument should contain `=`");
    let result = parse(second)?;
    out.push(format!("{first}={}", store_cb(second.to_owned())?));

    Ok(result)
}

fn parse_manifest_path(path: &str) -> Result<Option<PathBuf>> {
    let p = PathBuf::from(path);
    Ok(absolute_path(p).ok())
}

fn parse_target_dir(path: &str) -> Result<PathBuf> {
    absolute_path(PathBuf::from(path))
}

fn identity(arg: String) -> Result<String> {
    Ok(arg)
}

fn str_to_owned(arg: &str) -> Result<String> {
    Ok(arg.to_owned())
}

fn store_manifest_path(path: String) -> Result<String> {
    Path::new(&path).as_posix_relative()
}

fn store_target_dir(_: String) -> Result<String> {
    Ok("/target".to_owned())
}

// we have an optional parent to check for unresolvable recursion
fn parse_subcommand(
    arg: String,
    result: &mut Args,
    target_list: &TargetList,
    config: &CargoConfig,
    seen: &mut Vec<String>,
) -> Result<()> {
    if seen.iter().any(|x| x == &arg) {
        let chain = seen.join(" -> ");
        MessageInfo::default().fatal(
            format_args!("alias {arg} has unresolvable recursive definition: {chain} -> {arg}"),
            shell::ERROR_CODE,
        );
    }
    let subcommand = Subcommand::from(arg.as_ref());
    if subcommand == Subcommand::Other {
        if let Some(alias) = config.alias(&arg)? {
            seen.push(arg);
            let mut iter = alias.iter().cloned();
            while let Some(subarg) = iter.next() {
                parse_arg(subarg, result, target_list, config, seen, &mut iter)?;
            }
            return Ok(());
        }
        return Ok(());
    }

    // fallthrough
    result.all.push(arg);
    result.subcommand = Some(subcommand);

    Ok(())
}

fn parse_arg(
    arg: String,
    result: &mut Args,
    target_list: &TargetList,
    config: &CargoConfig,
    seen: &mut Vec<String>,
    iter: &mut impl Iterator<Item = String>,
) -> Result<()> {
    if arg.is_empty() {
        return Ok(());
    }
    if is_verbose(arg.as_str()) {
        result.verbose = true;
        result.all.push(arg);
    } else if matches!(arg.as_str(), "--version" | "-V") {
        result.version = true;
    } else if matches!(arg.as_str(), "--quiet" | "-q") {
        result.quiet = true;
        result.all.push(arg);
    } else if let Some(kind) = is_value_arg(&arg, "--color") {
        result.color = match kind {
            ArgKind::Next => {
                match parse_next_arg(arg, &mut result.all, str_to_owned, identity, iter)? {
                    Some(c) => Some(c),
                    None => shell::invalid_color(None),
                }
            }
            ArgKind::Equal => Some(parse_equal_arg(
                arg,
                &mut result.all,
                str_to_owned,
                identity,
            )?),
        };
    } else if let Some(kind) = is_value_arg(&arg, "--manifest-path") {
        result.manifest_path = match kind {
            ArgKind::Next => parse_next_arg(
                arg,
                &mut result.all,
                parse_manifest_path,
                store_manifest_path,
                iter,
            )?
            .flatten(),
            ArgKind::Equal => parse_equal_arg(
                arg,
                &mut result.all,
                parse_manifest_path,
                store_manifest_path,
            )?,
        };
    } else if let ("+", ch) = arg.split_at(1) {
        result.channel = Some(ch.to_owned());
    } else if let Some(kind) = is_value_arg(&arg, "--target") {
        let parse_target = |t: &str| Ok(Target::from(t, target_list));
        result.target = match kind {
            ArgKind::Next => parse_next_arg(arg, &mut result.all, parse_target, identity, iter)?,
            ArgKind::Equal => Some(parse_equal_arg(
                arg,
                &mut result.all,
                parse_target,
                identity,
            )?),
        };
    } else if let Some(kind) = is_value_arg(&arg, "--features") {
        match kind {
            ArgKind::Next => {
                let next = parse_next_arg(arg, &mut result.all, str_to_owned, identity, iter)?;
                if let Some(feature) = next {
                    result.features.push(feature);
                }
            }
            ArgKind::Equal => {
                result.features.push(parse_equal_arg(
                    arg,
                    &mut result.all,
                    str_to_owned,
                    identity,
                )?);
            }
        }
    } else if let Some(kind) = is_value_arg(&arg, "--target-dir") {
        match kind {
            ArgKind::Next => {
                result.target_dir = parse_next_arg(
                    arg,
                    &mut result.all,
                    parse_target_dir,
                    store_target_dir,
                    iter,
                )?;
            }
            ArgKind::Equal => {
                result.target_dir = Some(parse_equal_arg(
                    arg,
                    &mut result.all,
                    parse_target_dir,
                    store_target_dir,
                )?);
            }
        }
    } else if (!arg.starts_with('-') || arg == "--list") && result.subcommand.is_none() {
        parse_subcommand(arg, result, target_list, config, seen)?;
    } else {
        result.all.push(arg.clone());
    }

    Ok(())
}

pub fn parse(target_list: &TargetList, config: &CargoConfig) -> Result<Args> {
    let mut result = Args::default();
    let mut args = env::args().skip(1);
    let mut seen = vec![];
    while let Some(arg) = args.next() {
        parse_arg(arg, &mut result, target_list, config, &mut seen, &mut args)?;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustc;
    use crate::shell::Verbosity;

    macro_rules! s {
        ($s:literal) => {
            $s.to_owned()
        };
    }

    #[test]
    fn is_verbose_test() {
        assert!(!is_verbose("b"));
        assert!(!is_verbose("x"));
        assert!(!is_verbose("-"));
        assert!(!is_verbose("-V"));
        assert!(is_verbose("-v"));
        assert!(is_verbose("--verbose"));
        assert!(is_verbose("-vvvv"));
        assert!(!is_verbose("-version"));
    }

    #[test]
    fn test_nested_alias() {
        let mut args = Args::default();
        let target_list =
            rustc::target_list(&mut Verbosity::Quiet.into()).expect("failed to get target list");
        let config = CargoConfig::new(None);

        parse_subcommand(
            "g".to_owned(),
            &mut args,
            &target_list,
            &config,
            &mut vec![s!("x"), s!("y"), s!("z"), s!("a"), s!("e"), s!("f")],
        )
        .ok();
    }

    #[test]
    #[should_panic]
    fn test_recursive_alias() {
        let mut args = Args::default();
        let target_list =
            rustc::target_list(&mut Verbosity::Quiet.into()).expect("failed to get target list");
        let config = CargoConfig::new(None);

        parse_subcommand(
            "recursive".to_owned(),
            &mut args,
            &target_list,
            &config,
            &mut vec![s!("recursive")],
        )
        .ok();
    }

    #[test]
    #[should_panic]
    fn test_nested_recursive_alias() {
        let mut args = Args::default();
        let target_list =
            rustc::target_list(&mut Verbosity::Quiet.into()).expect("failed to get target list");
        let config = CargoConfig::new(None);

        parse_subcommand(
            "y".to_owned(),
            &mut args,
            &target_list,
            &config,
            &mut vec![s!("x"), s!("y"), s!("z"), s!("a")],
        )
        .ok();
    }
}
