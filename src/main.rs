#![deny(missing_debug_implementations, rust_2018_idioms)]

mod cargo;
mod cli;
mod config;
mod docker;
mod errors;
mod extensions;
mod file;
mod id;
mod interpreter;
mod rustc;
mod rustup;

use std::io::Write;
use std::process::ExitStatus;
use std::{env, io, process};

use config::Config;
use toml::{value::Table, Value};

use self::cargo::{Root, Subcommand};
use self::errors::*;
use self::rustc::{TargetList, VersionMetaExt};

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub enum Host {
    Other(String),

    // OSX
    X86_64AppleDarwin,
    // Support Apple Silicon, as developers are starting to use it as development workstation.
    Aarch64AppleDarwin,

    // Linux
    X86_64UnknownLinuxGnu,
    // Linux Aarch64 is become more popular in CI pipelines (e.g. to AWS Graviton based systems)
    Aarch64UnknownLinuxGnu,
    // (Alpine) Linux (musl) often use in CI pipelines to cross compile rust projects to different
    // targets (e.g. in GitLab CI pipelines).
    X86_64UnknownLinuxMusl,
    // (Alpine) Linux (musl) often use in CI pipelines to cross compile rust projects to different
    // targets (e.g. in GitLab CI pipelines). Now, that AWS Graviton based systems are gaining
    // attraction CI pipelines might run on (Alpine) Linux Aarch64.
    Aarch64UnknownLinuxMusl,

    // Windows MSVC
    X86_64PcWindowsMsvc,
}

impl Host {
    /// Checks if this `(host, target)` pair is supported by `cross`
    ///
    /// `target == None` means `target == host`
    fn is_supported(&self, target: Option<&Target>) -> bool {
        match std::env::var("CROSS_COMPATIBILITY_VERSION").as_ref().map(|v| v.as_str()) {
            // Old behavior (up to cross version 0.2.1) can be activated on demand using environment
            // variable `CROSS_COMPATIBILITY_VERSION`.
            Ok("0.2.1") => {
                match self {
                    Host::X86_64AppleDarwin | Host::Aarch64AppleDarwin => {
                        target.map(|t| t.needs_docker()).unwrap_or(false)
                    }
                    Host::X86_64UnknownLinuxGnu | Host::Aarch64UnknownLinuxGnu | Host::X86_64UnknownLinuxMusl | Host::Aarch64UnknownLinuxMusl => {
                        target.map(|t| t.needs_docker()).unwrap_or(true)
                    }
                    Host::X86_64PcWindowsMsvc => {
                        target.map(|t| t.triple() != Host::X86_64PcWindowsMsvc.triple() && t.needs_docker()).unwrap_or(false)
                    }
                    Host::Other(_) => false,
                }
            },
            // New behaviour, if a target is provided (--target ...) then always run with docker
            // image unless the target explicitly opts-out (i.e. unless needs_docker() returns false).
            // If no target is provided run natively (on host) using cargo.
            //
            // This not only simplifies the logic, it also enables forward-compatibility without
            // having to change cross every time someone comes up with the need for a new host/target
            // combination. It's totally fine to call cross with `--target=$host_triple`, for
            // example to test custom docker images. Cross should not try to recognize if host and
            // target are equal, it's a user decision and if user want's to bypass cross he can call
            // cargo directly or omit the `--target` option.
            _ => target.map(|t| t.needs_docker()).unwrap_or(false)
        }
    }

    fn triple(&self) -> &str {
        match self {
            Host::X86_64AppleDarwin => "x86_64-apple-darwin",
            Host::Aarch64AppleDarwin => "aarch64-apple-darwin",
            Host::X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            Host::Aarch64UnknownLinuxGnu => "aarch64-unknown-linux-gnu",
            Host::X86_64UnknownLinuxMusl => "x86_64-unknown-linux-musl",
            Host::Aarch64UnknownLinuxMusl => "aarch64-unknown-linux-musl",
            Host::X86_64PcWindowsMsvc => "x86_64-pc-windows-msvc",
            Host::Other(s) => s.as_str(),
        }
    }
}

impl<'a> From<&'a str> for Host {
    fn from(s: &str) -> Host {
        match s {
            "x86_64-apple-darwin" => Host::X86_64AppleDarwin,
            "x86_64-unknown-linux-gnu" => Host::X86_64UnknownLinuxGnu,
            "x86_64-unknown-linux-musl" => Host::X86_64UnknownLinuxMusl,
            "x86_64-pc-windows-msvc" => Host::X86_64PcWindowsMsvc,
            "aarch64-apple-darwin" => Host::Aarch64AppleDarwin,
            "aarch64-unknown-linux-gnu" => Host::Aarch64UnknownLinuxGnu,
            "aarch64-unknown-linux-musl" => Host::Aarch64UnknownLinuxMusl,
            s => Host::Other(s.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    BuiltIn { triple: String },
    Custom { triple: String },
}

impl Target {
    fn new_built_in(triple: &str) -> Self {
        Target::BuiltIn {
            triple: triple.to_owned(),
        }
    }

    fn new_custom(triple: &str) -> Self {
        Target::Custom {
            triple: triple.to_owned(),
        }
    }

    fn triple(&self) -> &str {
        match *self {
            Target::BuiltIn { ref triple } => triple,
            Target::Custom { ref triple } => triple,
        }
    }

    fn is_apple(&self) -> bool {
        self.triple().contains("apple")
    }

    fn is_bare_metal(&self) -> bool {
        self.triple().contains("thumb")
    }

    fn is_builtin(&self) -> bool {
        match *self {
            Target::BuiltIn { .. } => true,
            Target::Custom { .. } => false,
        }
    }

    fn is_bsd(&self) -> bool {
        self.triple().contains("bsd") || self.triple().contains("dragonfly")
    }

    fn is_solaris(&self) -> bool {
        self.triple().contains("solaris")
    }

    fn is_android(&self) -> bool {
        self.triple().contains("android")
    }

    fn is_emscripten(&self) -> bool {
        self.triple().contains("emscripten")
    }

    fn is_linux(&self) -> bool {
        self.triple().contains("linux") && !self.is_android()
    }

    fn is_windows(&self) -> bool {
        self.triple().contains("windows")
    }

    fn needs_docker(&self) -> bool {
        self.is_linux() || self.is_android() || self.is_bare_metal() || self.is_bsd() ||
        self.is_solaris() || !self.is_builtin() || self.is_windows() || self.is_emscripten() ||
        self.is_apple()
    }

    fn needs_interpreter(&self) -> bool {
        let native = self.triple().starts_with("x86_64")
            || self.triple().starts_with("i586")
            || self.triple().starts_with("i686");

        !native && (self.is_linux() || self.is_windows() || self.is_bare_metal())
    }
}

impl Target {
    fn from(triple: &str, target_list: &TargetList) -> Target {
        if target_list.contains(triple) {
            Target::new_built_in(triple)
        } else {
            Target::new_custom(triple)
        }
    }
}

impl From<Host> for Target {
    fn from(host: Host) -> Target {
        match host {
            Host::X86_64UnknownLinuxGnu => Target::new_built_in("x86_64-unknown-linux-gnu"),
            Host::X86_64UnknownLinuxMusl => Target::new_built_in("x86_64-unknown-linux-musl"),
            Host::X86_64AppleDarwin => Target::new_built_in("x86_64-apple-darwin"),
            Host::X86_64PcWindowsMsvc => Target::new_built_in("x86_64-pc-windows-msvc"),
            Host::Aarch64AppleDarwin => Target::new_built_in("aarch64-apple-darwin"),
            Host::Aarch64UnknownLinuxGnu => Target::new_built_in("aarch64-unknown-linux-gnu"),
            Host::Aarch64UnknownLinuxMusl => Target::new_built_in("aarch64-unknown-linux-musl"),
            Host::Other(s) => Target::from(s.as_str(), &rustc::target_list(false).unwrap()),
        }
    }
}

pub fn main() {
    fn show_backtrace() -> bool {
        env::var("RUST_BACKTRACE").as_ref().map(|s| &s[..]) == Ok("1")
    }

    match run() {
        Err(e) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();

            writeln!(stderr, "error: {}", e).ok();

            for e in e.iter().skip(1) {
                writeln!(stderr, "caused by: {}", e).ok();
            }

            if show_backtrace() {
                if let Some(backtrace) = e.backtrace() {
                    writeln!(stderr, "{:?}", backtrace).ok();
                }
            } else {
                writeln!(stderr, "note: run with `RUST_BACKTRACE=1` for a backtrace").ok();
            }

            process::exit(1)
        }
        Ok(status) => {
            if !status.success() {
                process::exit(status.code().unwrap_or(1))
            }
        }
    }
}

fn run() -> Result<ExitStatus> {
    let target_list = rustc::target_list(false)?;
    let args = cli::parse(&target_list);

    if args.all.iter().any(|a| a == "--version" || a == "-V") && args.subcommand.is_none() {
        println!(
            concat!("cross ", env!("CARGO_PKG_VERSION"), "{}"),
            include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt"))
        );
    }

    let verbose = args
        .all
        .iter()
        .any(|a| a == "--verbose" || a == "-v" || a == "-vv");

    let version_meta =
        rustc_version::version_meta().chain_err(|| "couldn'toml.t fetch the `rustc` version")?;
    if let Some(root) = cargo::root()? {
        let host = version_meta.host();

        if host.is_supported(args.target.as_ref()) {
            let target = args
                .target
                .unwrap_or_else(|| Target::from(host.triple(), &target_list));
            let toml = toml(&root)?;
            let config = Config::new(toml);

            let mut sysroot = rustc::sysroot(&host, &target, verbose)?;
            let default_toolchain = sysroot
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .ok_or("couldn't get toolchain name")?;
            let toolchain = if let Some(channel) = args.channel {
                [channel]
                    .iter()
                    .map(|c| c.as_str())
                    .chain(default_toolchain.splitn(2, '-').skip(1))
                    .collect::<Vec<_>>()
                    .join("-")
            } else {
                default_toolchain.to_string()
            };
            sysroot.set_file_name(&toolchain);

            let installed_toolchains = rustup::installed_toolchains(verbose)?;

            if !installed_toolchains.into_iter().any(|t| t == toolchain) {
                rustup::install_toolchain(&toolchain, verbose)?;
            }

            let available_targets = rustup::available_targets(&toolchain, verbose)?;
            let uses_xargo = config.xargo(&target)?
            .unwrap_or_else(|| !target.is_builtin() || !available_targets.contains(&target));

            if !uses_xargo
                && !available_targets.is_installed(&target)
                && available_targets.contains(&target)
            {
                rustup::install(&target, &toolchain, verbose)?;
            } else if !rustup::component_is_installed("rust-src", &toolchain, verbose)? {
                rustup::install_component("rust-src", &toolchain, verbose)?;
            }

            if args
                .subcommand
                .map(|sc| sc == Subcommand::Clippy)
                .unwrap_or(false)
                && !rustup::component_is_installed("clippy", &toolchain, verbose)?
            {
                rustup::install_component("clippy", &toolchain, verbose)?;
            }

            let needs_interpreter = args
                .subcommand
                .map(|sc| sc.needs_interpreter())
                .unwrap_or(false);

            let image_exists = match docker::image(&config, &target) {
                Ok(_) => true,
                Err(err) => {
                    eprintln!("Warning: {} Falling back to `cargo` on the host.", err);
                    false
                },
            };

            let filtered_args = if args
                .subcommand
                .map_or(false, |s| !s.needs_target_in_command())
            {
                let mut filtered_args = Vec::new();
                let mut args_iter = args.all.clone().into_iter();
                while let Some(arg) = args_iter.next() {
                    if arg == "--target" {
                        args_iter.next();
                    } else if arg.starts_with("--target=") {
                        // NOOP
                    } else {
                        filtered_args.push(arg)
                    }
                }
                filtered_args
            } else {
                args.all.clone()
            };

            if image_exists
                && target.needs_docker()
                && args.subcommand.map(|sc| sc.needs_docker()).unwrap_or(false)
            {
                if version_meta.needs_interpreter()
                    && needs_interpreter
                    && target.needs_interpreter()
                    && !interpreter::is_registered(&target)?
                {
                    docker::register(&target, verbose)?
                }

                return docker::run(
                    &target,
                    &filtered_args,
                    &args.target_dir,
                    &root,
                    &config,
                    uses_xargo,
                    &sysroot,
                    verbose,
                    args.docker_in_docker,
                );
            }
        }
    }

    cargo::run(&args.all, verbose)
}

/// Parsed `Cross.toml`
#[derive(Debug)]
pub struct Toml {
    table: Table,
}

impl Toml {
    /// Returns the `target.{}.image` part of `Cross.toml`
    pub fn image(&self, target: &Target) -> Result<Option<String>> {
        let triple = target.triple();

        if let Some(value) = self
            .table
            .get("target")
            .and_then(|t| t.get(triple))
            .and_then(|t| t.get("image"))
        {
            Ok(Some(value.as_str().ok_or_else(|| {
                format!("target.{}.image must be a string", triple)
            })?.to_string()))
        } else {
            Ok(None)
        }
    }

    /// Returns the `target.{}.runner` part of `Cross.toml`
    pub fn runner(&self, target: &Target) -> Result<Option<String>> {
        let triple = target.triple();

        if let Some(value) = self
            .table
            .get("target")
            .and_then(|t| t.get(triple))
            .and_then(|t| t.get("runner"))
        {
            let value = value
                .as_str()
                .ok_or_else(|| format!("target.{}.runner must be a string", triple))?.to_string();
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Returns the `build.image` or the `target.{}.xargo` part of `Cross.toml`
    pub fn xargo(&self, target: &Target) -> Result<(Option<bool>, Option<bool>)> {
        let triple = target.triple();

        if let Some(value) = self.table.get("build").and_then(|b| b.get("xargo")) {
            return Ok((Some(
                value
                    .as_bool()
                    .ok_or_else(|| "build.xargo must be a boolean")?,
            ), None));
        }

        if let Some(value) = self
            .table
            .get("target")
            .and_then(|b| b.get(triple))
            .and_then(|t| t.get("xargo"))
        {
            Ok((None, Some(value.as_bool().ok_or_else(|| {
                format!("target.{}.xargo must be a boolean", triple)
            })?)))
        } else {
            Ok((None, None))
        }
    }

    /// Returns the list of environment variables to pass through for `build`,
    pub fn env_passthrough_build(&self) -> Result<Vec<&str>> {
        self.build_env("passthrough")
    }

    /// Returns the list of environment variables to pass through for `target`,
    pub fn env_passthrough_target(&self, target: &Target) -> Result<Vec<&str>> {
        self.target_env(target, "passthrough")
    }
    
    /// Returns the list of environment variables to pass through for `build`,
    pub fn env_volumes_build(&self) -> Result<Vec<&str>> {
        self.build_env("volumes")
    }

    /// Returns the list of environment variables to pass through for `target`,
    pub fn env_volumes_target(&self, target: &Target) -> Result<Vec<&str>> {
        self.target_env(target, "volumes")
    }

    fn target_env(&self, target: &Target, key: &str) -> Result<Vec<&str>> {
        let triple = target.triple();

        match self
            .table
            .get("target")
            .and_then(|t| t.get(triple))
            .and_then(|t| t.get("env"))
            .and_then(|e| e.get(key))
        {
            Some(&Value::Array(ref vec)) => vec
                .iter()
                .map(|val| {
                    val.as_str().ok_or_else(|| {
                        format!(
                            "every target.{}.env.{} element must be a string",
                            triple, key
                        )
                        .into()
                    })
                })
                .collect(),
            _ => Ok(Vec::new()),
        }
    }

    fn build_env(&self, key: &str) -> Result<Vec<&str>> {
        match self
            .table
            .get("build")
            .and_then(|b| b.get("env"))
            .and_then(|e| e.get(key))
        {
            Some(&Value::Array(ref vec)) => vec
                .iter()
                .map(|val| {
                    val.as_str().ok_or_else(|| {
                        format!("every build.env.{} element must be a string", key).into()
                    })
                })
                .collect(),
            _ => Ok(Vec::new()),
        }
    }
}

/// Parses the `Cross.toml` at the root of the Cargo project (if any)
fn toml(root: &Root) -> Result<Option<Toml>> {
    let path = root.path().join("Cross.toml");

    if path.exists() {
        Ok(Some(Toml {
            table: if let Ok(Value::Table(table)) = file::read(&path)?.parse() {
                table
            } else {
                return Err(format!("couldn't parse {} as TOML table", path.display()).into());
            },
        }))
    } else {
        Ok(None)
    }
}
