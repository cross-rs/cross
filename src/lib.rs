//! [![crates.io](https://img.shields.io/crates/v/cross.svg)](https://crates.io/crates/cross)
//! [![crates.io](https://img.shields.io/crates/d/cross.svg)](https://crates.io/crates/cross)
//! [![Matrix](https://img.shields.io/matrix/cross-rs:matrix.org)](https://matrix.to/#/#cross-rs:matrix.org)
//!
//! # `cross`
//!
//! <p style="background:rgba(148,192,255,0.1);padding:0.5em;border-radius:0.2em">
//! <strong>⚠️ Warning:</strong> The cross library is for internal
//! use only: only the command-line interface is stable. The library
//! may change at any point for any reason. For documentation on the
//! CLI, please see the repository <a href="https://github.com/cross-rs/cross">README</a>
//! or the <a href="https://github.com/cross-rs/cross/wiki">wiki</a>.
//! </p>

#![deny(missing_debug_implementations, rust_2018_idioms)]

#[cfg(test)]
mod tests;

mod cargo;
mod cli;
mod config;
mod cross_toml;
mod docker;
mod errors;
mod extensions;
mod file;
mod id;
mod interpreter;
mod rustc;
mod rustup;

use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitStatus;

use config::Config;
use rustc_version::Channel;
use serde::Deserialize;

pub use self::cargo::{cargo_metadata_with_args, CargoMetadata, Subcommand};
use self::cross_toml::CrossToml;
use self::errors::Context;
use self::rustc::{TargetList, VersionMetaExt};

pub use self::docker::get_container_engine;
pub use self::docker::CROSS_IMAGE;
pub use self::errors::{install_panic_hook, Result};
pub use self::extensions::{CommandExt, OutputExt};

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
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
        match std::env::var("CROSS_COMPATIBILITY_VERSION")
            .as_ref()
            .map(|v| v.as_str())
        {
            // Old behavior (up to cross version 0.2.1) can be activated on demand using environment
            // variable `CROSS_COMPATIBILITY_VERSION`.
            Ok("0.2.1") => match self {
                Host::X86_64AppleDarwin | Host::Aarch64AppleDarwin => {
                    target.map(|t| t.needs_docker()).unwrap_or(false)
                }
                Host::X86_64UnknownLinuxGnu
                | Host::Aarch64UnknownLinuxGnu
                | Host::X86_64UnknownLinuxMusl
                | Host::Aarch64UnknownLinuxMusl => target.map(|t| t.needs_docker()).unwrap_or(true),
                Host::X86_64PcWindowsMsvc => target
                    .map(|t| t.triple() != Host::X86_64PcWindowsMsvc.triple() && t.needs_docker())
                    .unwrap_or(false),
                Host::Other(_) => false,
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
            _ => target.map(|t| t.needs_docker()).unwrap_or(false),
        }
    }

    /// Returns the [`Target`] as target triple string
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(from = "&str")]
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
        self.is_linux()
            || self.is_android()
            || self.is_bare_metal()
            || self.is_bsd()
            || self.is_solaris()
            || !self.is_builtin()
            || self.is_windows()
            || self.is_emscripten()
            || self.is_apple()
    }

    fn needs_interpreter(&self) -> bool {
        let native = self.triple().starts_with("x86_64")
            || self.triple().starts_with("i586")
            || self.triple().starts_with("i686");

        !native && (self.is_linux() || self.is_windows() || self.is_bare_metal())
    }

    fn needs_docker_seccomp(&self) -> bool {
        let arch_32bit = self.triple().starts_with("arm")
            || self.triple().starts_with("thumb")
            || self.triple().starts_with("i586")
            || self.triple().starts_with("i686");

        arch_32bit && self.is_android()
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.triple())
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

impl From<&str> for Target {
    fn from(target_str: &str) -> Target {
        let target_host: Host = target_str.into();
        target_host.into()
    }
}

pub fn run() -> Result<ExitStatus> {
    let target_list = rustc::target_list(false)?;
    let args = cli::parse(&target_list)?;

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

    let host_version_meta =
        rustc_version::version_meta().wrap_err("couldn't fetch the `rustc` version")?;
    let cwd = std::env::current_dir()?;
    if let Some(metadata) = cargo_metadata_with_args(None, Some(&args), verbose)? {
        let host = host_version_meta.host();
        let toml = toml(&metadata)?;
        let config = Config::new(toml);
        let target = args
            .target
            .or_else(|| config.target(&target_list))
            .unwrap_or_else(|| Target::from(host.triple(), &target_list));
        config.confusable_target(&target);

        let image_exists = match docker::image(&config, &target) {
            Ok(_) => true,
            Err(err) => {
                eprintln!("Warning: {}", err);
                false
            }
        };

        if image_exists && host.is_supported(Some(&target)) {
            let mut sysroot = rustc::sysroot(&host, &target, verbose)?;
            let default_toolchain = sysroot
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .ok_or_else(|| eyre::eyre!("couldn't get toolchain name"))?;
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
            let mut is_nightly = toolchain.contains("nightly");

            let installed_toolchains = rustup::installed_toolchains(verbose)?;

            if !installed_toolchains.into_iter().any(|t| t == toolchain) {
                rustup::install_toolchain(&toolchain, verbose)?;
            }
            // TODO: Provide a way to pick/match the toolchain version as a consumer of `cross`.
            if let Some((rustc_version, channel, rustc_commit)) = rustup::rustc_version(&sysroot)? {
                warn_host_version_mismatch(
                    &host_version_meta,
                    &toolchain,
                    &rustc_version,
                    &rustc_commit,
                )?;
                is_nightly = channel == Channel::Nightly;
            }

            let available_targets = rustup::available_targets(&toolchain, verbose)?;
            let uses_xargo = config
                .xargo(&target)
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

            let mut filtered_args = if args
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
            // Make sure --target is present
            } else if !args.all.iter().any(|a| a.starts_with("--target")) {
                let mut args_with_target = args.all.clone();
                args_with_target.push("--target".to_string());
                args_with_target.push(target.triple().to_string());
                args_with_target
            } else {
                args.all.clone()
            };

            let is_test = args
                .subcommand
                .map(|sc| sc == Subcommand::Test)
                .unwrap_or(false);
            if is_test && args.enable_doctests && is_nightly {
                filtered_args.push("-Zdoctest-xcompile".to_string());
            }

            if target.needs_docker() && args.subcommand.map(|sc| sc.needs_docker()).unwrap_or(false)
            {
                if host_version_meta.needs_interpreter()
                    && needs_interpreter
                    && target.needs_interpreter()
                    && !interpreter::is_registered(&target)?
                {
                    docker::register(&target, verbose)?
                }

                return docker::run(
                    &target,
                    &filtered_args,
                    &metadata,
                    &config,
                    uses_xargo,
                    &sysroot,
                    verbose,
                    args.docker_in_docker,
                    &cwd,
                );
            }
        }
    }

    // if we fallback to the host cargo, use the same invocation that was made to cross
    let argv: Vec<String> = env::args().skip(1).collect();
    eprintln!("Warning: Falling back to `cargo` on the host.");
    match args.subcommand {
        Some(Subcommand::List) => {
            // this won't print in order if we have both stdout and stderr.
            let out = cargo::run_and_get_output(&argv, verbose)?;
            let stdout = out.stdout()?;
            if out.status.success() && cli::is_subcommand_list(&stdout) {
                cli::fmt_subcommands(&stdout);
            } else {
                // Not a list subcommand, which can happen with weird edge-cases.
                print!("{}", stdout);
                io::stdout().flush().unwrap();
            }
            Ok(out.status)
        }
        _ => cargo::run(&argv, verbose),
    }
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum VersionMatch {
    Same,
    OlderTarget,
    NewerTarget,
    Different,
}

pub(crate) fn warn_host_version_mismatch(
    host_version_meta: &rustc_version::VersionMeta,
    toolchain: &str,
    rustc_version: &rustc_version::Version,
    rustc_commit: &str,
) -> Result<VersionMatch> {
    let host_commit = (&host_version_meta.short_version_string)
        .splitn(3, ' ')
        .nth(2);
    let rustc_commit_date = rustc_commit
        .split_once(' ')
        .and_then(|x| x.1.strip_suffix(')'));

    // This should only hit on non Host::X86_64UnknownLinuxGnu hosts
    if rustc_version != &host_version_meta.semver || (Some(rustc_commit) != host_commit) {
        let versions = rustc_version.cmp(&host_version_meta.semver);
        let dates = rustc_commit_date.cmp(&host_version_meta.commit_date.as_deref());

        let rustc_warning = format!(
            "rustc `{rustc_version} {rustc_commit}` for the target. Current active rustc on the host is `{}`",
            host_version_meta.short_version_string
        );
        if versions.is_lt() || (versions.is_eq() && dates.is_lt()) {
            eprintln!("Warning: using older {rustc_warning}.\n > Update with `rustup update --force-non-host {toolchain}`");
            return Ok(VersionMatch::OlderTarget);
        } else if versions.is_gt() || (versions.is_eq() && dates.is_gt()) {
            eprintln!("Warning: using newer {rustc_warning}.\n > Update with `rustup update`");
            return Ok(VersionMatch::NewerTarget);
        } else {
            eprintln!("Warning: using {rustc_warning}.");
            return Ok(VersionMatch::Different);
        }
    }
    Ok(VersionMatch::Same)
}

/// Parses the `Cross.toml` at the root of the Cargo project or from the
/// `CROSS_CONFIG` environment variable (if any exist in either location).
fn toml(metadata: &CargoMetadata) -> Result<Option<CrossToml>> {
    let path = match env::var("CROSS_CONFIG") {
        Ok(var) => PathBuf::from(var),
        Err(_) => metadata.workspace_root.join("Cross.toml"),
    };

    if path.exists() {
        let content = file::read(&path)
            .wrap_err_with(|| format!("could not read file `{}`", path.display()))?;

        let (config, _) = CrossToml::parse(&content)
            .wrap_err_with(|| format!("failed to parse file `{}` as TOML", path.display()))?;

        Ok(Some(config))
    } else {
        // Checks if there is a lowercase version of this file
        if metadata.workspace_root.join("cross.toml").exists() {
            eprintln!("There's a file named cross.toml, instead of Cross.toml. You may want to rename it, or it won't be considered.");
        }
        Ok(None)
    }
}
