use std::path::{Path, PathBuf};
use std::{env, fmt};

use crate::cargo::Subcommand;
use crate::errors::Result;
use crate::file::{absolute_path, PathExt};
use crate::rustc::TargetList;
use crate::shell::{MessageInfo, COLORS};
use crate::Target;

/// An option for a CLI flag value.
#[derive(Debug)]
pub enum FlagOption<T> {
    /// Value for flag was not provided.
    Some(T),
    /// Flag was not provided.
    None,
    /// Flag was provided multiple times.
    Double,
    /// Flag was not provided.
    Missing,
}

impl<T> FlagOption<T> {
    pub fn is_some(&self) -> bool {
        matches!(self, Self::Some(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::Some(_))
    }

    pub fn set(&mut self, other: Self) {
        *self = match self {
            FlagOption::None => other,
            _ => FlagOption::Double,
        };
    }

    pub fn as_ref(&self) -> FlagOption<&T> {
        match self {
            FlagOption::Some(ref value) => FlagOption::Some(value),
            FlagOption::None => FlagOption::None,
            FlagOption::Double => FlagOption::Double,
            FlagOption::Missing => FlagOption::Missing,
        }
    }

    pub fn as_mut(&mut self) -> FlagOption<&mut T> {
        match self {
            FlagOption::Some(ref mut value) => FlagOption::Some(value),
            FlagOption::None => FlagOption::None,
            FlagOption::Double => FlagOption::Double,
            FlagOption::Missing => FlagOption::Missing,
        }
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> FlagOption<U> {
        match self {
            FlagOption::Some(x) => FlagOption::Some(f(x)),
            FlagOption::None => FlagOption::None,
            FlagOption::Double => FlagOption::Double,
            FlagOption::Missing => FlagOption::Missing,
        }
    }

    /// Consume self and report a fatal error message if missing or doubled up
    pub fn to_option<Message: fmt::Display>(
        self,
        arg: Message,
        possible: Option<&[&str]>,
        code: i32,
    ) -> Option<T> {
        let mut msg_info = MessageInfo::default();
        match self {
            FlagOption::Some(value) => Some(value),
            FlagOption::None => None,
            FlagOption::Double => msg_info.double_fatal_usage(arg, code),
            FlagOption::Missing => msg_info.invalid_fatal_usage(arg, None, possible, code),
        }
    }
}

impl<T> FlagOption<Option<T>> {
    pub fn flatten(self) -> FlagOption<T> {
        match self {
            FlagOption::Some(value) => value.into(),
            FlagOption::None => FlagOption::None,
            FlagOption::Double => FlagOption::Double,
            FlagOption::Missing => FlagOption::Missing,
        }
    }
}

impl<T> From<Option<T>> for FlagOption<T> {
    fn from(option: Option<T>) -> Self {
        match option {
            Some(value) => FlagOption::Some(value),
            None => FlagOption::None,
        }
    }
}

#[derive(Debug)]
pub struct Args {
    pub cargo_args: Vec<String>,
    pub rest_args: Vec<String>,
    pub features: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub channel: Option<String>,
    pub target: Option<Target>,
    pub target_dir: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub color: Option<String>,
    pub verbose: u8,
    pub quiet: bool,
}

/// Internal implementation of args for the parsing, that handles
/// missing values for flags and flags which are provided multiple times.
#[derive(Debug)]
pub struct ArgsImpl {
    pub cargo_args: Vec<String>,
    pub rest_args: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub channel: Option<String>,
    pub target: FlagOption<Target>,
    pub target_dir: FlagOption<PathBuf>,
    pub manifest_path: FlagOption<PathBuf>,
    pub color: FlagOption<String>,
    pub features: FlagOption<Vec<String>>,
    pub verbose: u8,
    pub quiet: bool,
}

impl ArgsImpl {
    pub const fn new() -> Self {
        Self {
            cargo_args: Vec::new(),
            rest_args: Vec::new(),
            subcommand: None,
            channel: None,
            target: FlagOption::None,
            target_dir: FlagOption::None,
            manifest_path: FlagOption::None,
            color: FlagOption::None,
            features: FlagOption::None,
            verbose: 0,
            quiet: false,
        }
    }

    fn into_args(self) -> Args {
        // FIXME: cargo always goes right-to-left when printing missing
        // or doubled arguments, however, we print in a fixed order.
        Args {
            cargo_args: self.cargo_args,
            rest_args: self.rest_args,
            subcommand: self.subcommand,
            channel: self.channel,
            target: self.target.to_option("--target <TRIPLE>", None, 1),
            target_dir: self
                .target_dir
                .to_option("--target-dir <DIRECTORY>", None, 1),
            manifest_path: self
                .manifest_path
                .to_option("--manifest-path <PATH>", None, 1),
            color: self.color.to_option("--color <WHEN>", Some(COLORS), 1),
            features: self
                .features
                .to_option("--features <FEATURES>", None, 1)
                .unwrap_or_default(),
            verbose: self.verbose,
            quiet: self.quiet,
        }
    }
}

impl Default for ArgsImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
enum ArgKind {
    Next,
    Equal,
}

#[derive(Debug)]
struct Parser<'a, Iter: Iterator<Item = String>> {
    result: ArgsImpl,
    input: Iter,
    target_list: &'a TargetList,
}

impl<'a, Iter: Iterator<Item = String>> Parser<'a, Iter> {
    fn parse_channel(arg: &str) -> Option<String> {
        match arg.split_at(1) {
            ("+", channel) => Some(channel.to_owned()),
            _ => None,
        }
    }

    fn parse_color(&mut self, arg: String, kind: ArgKind) -> Result<()> {
        let color = match kind {
            ArgKind::Next => self.parse_flag_next(arg, str_to_owned, identity)?,
            ArgKind::Equal => {
                FlagOption::Some(self.parse_flag_equal(arg, str_to_owned, identity)?)
            }
        };
        self.result.color.set(color);

        Ok(())
    }

    fn parse_manifest(&mut self, arg: String, kind: ArgKind) -> Result<()> {
        let manifest_path = match kind {
            ArgKind::Next => self
                .parse_flag_next(arg, parse_manifest_path, store_manifest_path)?
                .flatten(),
            ArgKind::Equal => self
                .parse_flag_equal(arg, parse_manifest_path, store_manifest_path)?
                .into(),
        };
        self.result.manifest_path.set(manifest_path);

        Ok(())
    }

    fn parse_target(&mut self, arg: String, kind: ArgKind) -> Result<()> {
        let into = |t: &str| Ok(Target::from(t, self.target_list));
        let target = match kind {
            ArgKind::Next => self.parse_flag_next(arg, into, identity)?,
            ArgKind::Equal => FlagOption::Some(self.parse_flag_equal(arg, into, identity)?),
        };
        self.result.target.set(target);

        Ok(())
    }

    fn parse_explain(&mut self, arg: String, kind: ArgKind) -> Result<()> {
        let subcommand = Subcommand::Explain;
        self.result.subcommand = Some(match self.result.subcommand {
            Some(ref mut sc) => sc.set(subcommand),
            None => subcommand,
        });
        self.parse_flag_value(arg, kind)
    }

    fn parse_target_dir(&mut self, arg: String, kind: ArgKind) -> Result<()> {
        let target_dir = match kind {
            ArgKind::Next => self.parse_flag_next(arg, parse_target_dir, store_target_dir)?,
            ArgKind::Equal => {
                FlagOption::Some(self.parse_flag_equal(arg, parse_target_dir, store_target_dir)?)
            }
        };

        self.result.target_dir.set(target_dir);

        Ok(())
    }

    fn parse_features(&mut self, arg: String, kind: ArgKind) -> Result<()> {
        let features = match kind {
            ArgKind::Next => self.parse_flag_next(arg, str_to_owned, identity)?,
            ArgKind::Equal => {
                FlagOption::Some(self.parse_flag_equal(arg, str_to_owned, identity)?)
            }
        };

        let features = features.map(|x| match x.is_empty() {
            true => vec![],
            false => vec![x],
        });
        match (self.result.features.as_mut(), features) {
            (FlagOption::Some(lhs), FlagOption::Some(mut rhs)) => lhs.append(&mut rhs),
            (FlagOption::Some(_), _) => (),
            (_, rhs) => self.result.features = rhs,
        }

        Ok(())
    }

    fn parse_flag_value(&mut self, arg: String, kind: ArgKind) -> Result<()> {
        self.result.cargo_args.push(arg);
        match kind {
            ArgKind::Next => match self.input.next() {
                Some(next) if is_flag(&next) => self.parse_flag(next)?,
                Some(next) => self.result.cargo_args.push(next),
                None => (),
            },
            ArgKind::Equal => (),
        }

        Ok(())
    }

    fn parse_flag_next<T>(
        &mut self,
        arg: String,
        parse: impl Fn(&str) -> Result<T>,
        store: impl Fn(String) -> Result<String>,
    ) -> Result<FlagOption<T>> {
        // `--$flag $value` does not support flag-like values: `--target-dir --x` is invalid
        self.result.cargo_args.push(arg);
        match self.input.next() {
            Some(next) if is_flag(&next) => {
                self.parse_flag(next)?;
                Ok(FlagOption::Missing)
            }
            Some(next) => {
                let parsed = parse(&next)?;
                self.result.cargo_args.push(store(next)?);
                Ok(FlagOption::Some(parsed))
            }
            None => Ok(FlagOption::Missing),
        }
    }

    fn parse_flag_equal<T>(
        &mut self,
        arg: String,
        parse: impl Fn(&str) -> Result<T>,
        store: impl Fn(String) -> Result<String>,
    ) -> Result<T> {
        // `--$flag=$value` supports flag-like values: `--target-dir=--x` is valid
        let (first, second) = arg.split_once('=').expect("argument should contain `=`");
        let parsed = parse(second)?;
        self.result
            .cargo_args
            .push(format!("{first}={}", store(second.to_owned())?));

        Ok(parsed)
    }

    fn parse_flag(&mut self, arg: String) -> Result<()> {
        // we only consider flags accepted by `cargo` itself or `--help`,
        // and we will ignore any subcommand value. this is fine
        // since we will already have a subcommand, and since `--help` is
        // a flag, we will never accidentally parse it as a value.
        if arg == "--" {
            self.result.rest_args.push(arg);
            self.result.rest_args.extend(self.input.by_ref());
        } else if let v @ 1.. = is_verbose(arg.as_str()) {
            self.result.verbose += v;
            self.result.cargo_args.push(arg);
        } else if matches!(arg.as_str(), "--quiet" | "-q") {
            self.result.quiet = true;
            self.result.cargo_args.push(arg);
        } else if let Some(kind) = is_value_arg(&arg, "--color") {
            self.parse_color(arg, kind)?;
        } else if let Some(kind) = is_value_arg(&arg, "--manifest-path") {
            self.parse_manifest(arg, kind)?;
        } else if let Some(kind) = is_value_arg(&arg, "--target-dir") {
            self.parse_target_dir(arg, kind)?;
        } else if let Some(kind) = is_value_arg(&arg, "--explain") {
            self.parse_explain(arg, kind)?;
        } else if let Some(kind) = is_value_arg(&arg, "--config") {
            self.parse_flag_value(arg, kind)?;
        } else if let Some(kind) = is_value_arg(&arg, "-Z") {
            self.parse_flag_value(arg, kind)?;
        } else if let Some(kind) = is_value_arg(&arg, "--target") {
            self.parse_target(arg, kind)?;
        } else if let Some(kind) = is_value_arg(&arg, "--features") {
            self.parse_features(arg, kind)?;
        } else if matches!(arg.as_str(), "--help" | "--list" | "--version" | "-V") {
            let subcommand = Subcommand::from(arg.as_str());
            self.result.subcommand = Some(match self.result.subcommand {
                Some(ref mut sc) => sc.set(subcommand),
                None => subcommand,
            });
            self.result.cargo_args.push(arg);
        } else {
            self.result.cargo_args.push(arg);
        }

        Ok(())
    }

    fn parse_value(&mut self, arg: String) -> Result<()> {
        let subcommand = Subcommand::from(arg.as_ref());
        self.result.subcommand = Some(match self.result.subcommand {
            Some(ref mut sc) => sc.set(subcommand),
            None => subcommand,
        });
        self.result.cargo_args.push(arg);

        Ok(())
    }

    fn parse_next(&mut self, arg: String) -> Result<()> {
        if arg.is_empty() {
            Ok(())
        } else if let Some(channel) = Self::parse_channel(&arg) {
            self.result.channel = Some(channel);
            Ok(())
        } else if is_flag(&arg) {
            self.parse_flag(arg)
        } else {
            self.parse_value(arg)
        }
    }

    fn parse(&mut self) -> Result<()> {
        while let Some(arg) = self.input.next() {
            self.parse_next(arg)?;
        }
        Ok(())
    }
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
        for line in &host {
            msg_info.print(line)?;
        }
    }
    Ok(())
}

fn is_verbose(arg: &str) -> u8 {
    match arg {
        "--verbose" => 1,
        // cargo can handle any number of "v"s
        a => {
            if a.starts_with('-')
                && a.len() >= 2
                && a.get(1..)
                    .map(|a| a.chars().all(|x| x == 'v'))
                    .unwrap_or_default()
            {
                // string must be of form `-v[v]*` here
                a.len() as u8 - 1
            } else {
                0
            }
        }
    }
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

fn is_flag(arg: &str) -> bool {
    arg.starts_with('-')
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

pub fn parse(target_list: &TargetList) -> Result<Args> {
    let result = ArgsImpl::new();
    let input = env::args().skip(1);
    let mut parser = Parser {
        result,
        input,
        target_list,
    };
    parser.parse()?;

    Ok(parser.result.into_args())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_verbose_test() {
        assert!(is_verbose("b") == 0);
        assert!(is_verbose("x") == 0);
        assert!(is_verbose("-") == 0);
        assert!(is_verbose("-V") == 0);
        assert!(is_verbose("-v") == 1);
        assert!(is_verbose("--verbose") == 1);
        assert!(is_verbose("-vvvv") == 4);
        assert!(is_verbose("-version") == 0);
    }
}
