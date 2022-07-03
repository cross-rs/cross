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
pub mod docker;
pub mod errors;
mod extensions;
mod file;
mod id;
mod interpreter;
pub mod rustc;
mod rustup;
pub mod shell;
pub mod temp;

use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitStatus;

use config::Config;
use rustc_version::Channel;
use serde::{Deserialize, Serialize, Serializer};

pub use self::cargo::{cargo_command, cargo_metadata_with_args, CargoMetadata, Subcommand};
use self::cross_toml::CrossToml;
use self::errors::Context;
use self::shell::{MessageInfo, Verbosity};

pub use self::errors::{install_panic_hook, install_termination_hook, Result};
pub use self::extensions::{CommandExt, OutputExt};
pub use self::file::{pretty_path, ToUtf8};
pub use self::rustc::{TargetList, VersionMetaExt};

pub const CROSS_LABEL_DOMAIN: &str = "org.cross-rs";

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
    pub fn triple(&self) -> &str {
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
#[serde(from = "String")]
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

    /// Returns the architecture name according to `dpkg` naming convention
    ///
    /// # Notes
    ///
    /// Some of these make no sense to use in our standard images
    pub fn deb_arch(&self) -> Option<&'static str> {
        match self.triple() {
            "aarch64-unknown-linux-gnu" => Some("arm64"),
            "aarch64-unknown-linux-musl" => Some("musl-linux-arm64"),
            "aarch64-linux-android" => None,
            "x86_64-unknown-linux-gnu" => Some("amd64"),
            "x86_64-apple-darwin" => Some("darwin-amd64"),
            "x86_64-unknown-linux-musl" => Some("musl-linux-amd64"),

            "x86_64-pc-windows-msvc" => None,
            "arm-unknown-linux-gnueabi" => Some("armel"),
            "arm-unknown-linux-gnueabihf" => Some("armhf"),
            "armv7-unknown-linux-gnueabi" => Some("armel"),
            "armv7-unknown-linux-gnueabihf" => Some("armhf"),
            "thumbv7neon-unknown-linux-gnueabihf" => Some("armhf"),
            "i586-unknown-linux-gnu" => Some("i386"),
            "i686-unknown-linux-gnu" => Some("i386"),
            "mips-unknown-linux-gnu" => Some("mips"),
            "mipsel-unknown-linux-gnu" => Some("mipsel"),
            "mips64-unknown-linux-gnuabi64" => Some("mips64"),
            "mips64el-unknown-linux-gnuabi64" => Some("mips64el"),
            "mips64-unknown-linux-muslabi64" => Some("musl-linux-mips64"),
            "mips64el-unknown-linux-muslabi64" => Some("musl-linux-mips64el"),
            "powerpc-unknown-linux-gnu" => Some("powerpc"),
            "powerpc64-unknown-linux-gnu" => Some("ppc64"),
            "powerpc64le-unknown-linux-gnu" => Some("ppc64el"),
            "riscv64gc-unknown-linux-gnu" => Some("riscv64"),
            "s390x-unknown-linux-gnu" => Some("s390x"),
            "sparc64-unknown-linux-gnu" => Some("sparc64"),
            "arm-unknown-linux-musleabihf" => Some("musl-linux-armhf"),
            "arm-unknown-linux-musleabi" => Some("musl-linux-arm"),
            "armv5te-unknown-linux-gnueabi" => None,
            "armv5te-unknown-linux-musleabi" => None,
            "armv7-unknown-linux-musleabi" => Some("musl-linux-arm"),
            "armv7-unknown-linux-musleabihf" => Some("musl-linux-armhf"),
            "i586-unknown-linux-musl" => Some("musl-linux-i386"),
            "i686-unknown-linux-musl" => Some("musl-linux-i386"),
            "mips-unknown-linux-musl" => Some("musl-linux-mips"),
            "mipsel-unknown-linux-musl" => Some("musl-linux-mipsel"),
            "arm-linux-androideabi" => None,
            "armv7-linux-androideabi" => None,
            "thumbv7neon-linux-androideabi" => None,
            "i686-linux-android" => None,
            "x86_64-linux-android" => None,
            "x86_64-pc-windows-gnu" => None,
            "i686-pc-windows-gnu" => None,
            "asmjs-unknown-emscripten" => None,
            "wasm32-unknown-emscripten" => None,
            "x86_64-unknown-dragonfly" => Some("dragonflybsd-amd64"),
            "i686-unknown-freebsd" => Some("freebsd-i386"),
            "x86_64-unknown-freebsd" => Some("freebsd-amd64"),
            "x86_64-unknown-netbsd" => Some("netbsd-amd64"),
            "sparcv9-sun-solaris" => Some("solaris-sparc"),
            "x86_64-sun-solaris" => Some("solaris-amd64"),
            "thumbv6m-none-eabi" => Some("arm"),
            "thumbv7em-none-eabi" => Some("arm"),
            "thumbv7em-none-eabihf" => Some("armhf"),
            "thumbv7m-none-eabi" => Some("arm"),
            _ => None,
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.triple())
    }
}

impl Target {
    pub fn from(triple: &str, target_list: &TargetList) -> Target {
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
            Host::Other(s) => Target::from(
                s.as_str(),
                &rustc::target_list(Verbosity::Quiet.into()).unwrap(),
            ),
        }
    }
}

impl From<String> for Target {
    fn from(target_str: String) -> Target {
        let target_host: Host = target_str.as_str().into();
        target_host.into()
    }
}

impl Serialize for Target {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Target::BuiltIn { triple } => serializer.serialize_str(triple),
            Target::Custom { triple } => serializer.serialize_str(triple),
        }
    }
}

fn warn_on_failure(target: &Target, toolchain: &str, msg_info: MessageInfo) -> Result<()> {
    let rust_std = format!("rust-std-{target}");
    if target.is_builtin() {
        let component = rustup::check_component(&rust_std, toolchain, msg_info)?;
        if component.is_not_available() {
            shell::warn(format!("rust-std is not available for {target}"), msg_info)?;
            shell::note(
                format_args!(
                    r#"you may need to build components for the target via `-Z build-std=<components>` or in your cross configuration specify `target.{target}.build-std`
              the available components are core, std, alloc, and proc_macro"#
                ),
                msg_info,
            )?;
        }
    }
    Ok(())
}

pub fn run() -> Result<ExitStatus> {
    let target_list = rustc::target_list(Verbosity::Quiet.into())?;
    let args = cli::parse(&target_list)?;

    if args.version && args.subcommand.is_none() {
        let commit_info = include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt"));
        shell::print(
            format!(
                concat!("cross ", env!("CARGO_PKG_VERSION"), "{}"),
                commit_info
            ),
            args.msg_info,
        )?;
    }

    let host_version_meta = rustc::version_meta()?;
    let cwd = std::env::current_dir()?;
    if let Some(metadata) = cargo_metadata_with_args(None, Some(&args), args.msg_info)? {
        let host = host_version_meta.host();
        let toml = toml(&metadata, args.msg_info)?;
        let config = Config::new(toml);
        let target = args
            .target
            .or_else(|| config.target(&target_list))
            .unwrap_or_else(|| Target::from(host.triple(), &target_list));
        config.confusable_target(&target, args.msg_info)?;

        let image_exists = match docker::image_name(&config, &target) {
            Ok(_) => true,
            Err(err) => {
                shell::warn(err, args.msg_info)?;
                false
            }
        };

        if image_exists && host.is_supported(Some(&target)) {
            let (toolchain, sysroot) =
                rustc::get_sysroot(&host, &target, args.channel.as_deref(), args.msg_info)?;
            let mut is_nightly = toolchain.contains("nightly");

            let installed_toolchains = rustup::installed_toolchains(args.msg_info)?;

            if !installed_toolchains.into_iter().any(|t| t == toolchain) {
                rustup::install_toolchain(&toolchain, args.msg_info)?;
            }
            // TODO: Provide a way to pick/match the toolchain version as a consumer of `cross`.
            if let Some((rustc_version, channel, rustc_commit)) = rustup::rustc_version(&sysroot)? {
                warn_host_version_mismatch(
                    &host_version_meta,
                    &toolchain,
                    &rustc_version,
                    &rustc_commit,
                    args.msg_info,
                )?;
                is_nightly = channel == Channel::Nightly;
            }

            let uses_build_std = config.build_std(&target).unwrap_or(false);
            let uses_xargo =
                !uses_build_std && config.xargo(&target).unwrap_or(!target.is_builtin());
            if std::env::var("CROSS_CUSTOM_TOOLCHAIN").is_err() {
                // build-std overrides xargo, but only use it if it's a built-in
                // tool but not an available target or doesn't have rust-std.
                let available_targets = rustup::available_targets(&toolchain, args.msg_info)?;

                if !is_nightly && uses_build_std {
                    eyre::bail!(
                        "no rust-std component available for {}: must use nightly",
                        target.triple()
                    );
                }

                if !uses_xargo
                    && !available_targets.is_installed(&target)
                    && available_targets.contains(&target)
                {
                    rustup::install(&target, &toolchain, args.msg_info)?;
                } else if !rustup::component_is_installed("rust-src", &toolchain, args.msg_info)? {
                    rustup::install_component("rust-src", &toolchain, args.msg_info)?;
                }
                if args
                    .subcommand
                    .map(|sc| sc == Subcommand::Clippy)
                    .unwrap_or(false)
                    && !rustup::component_is_installed("clippy", &toolchain, args.msg_info)?
                {
                    rustup::install_component("clippy", &toolchain, args.msg_info)?;
                }
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
            if uses_build_std {
                filtered_args.push("-Zbuild-std".to_string());
            }

            let is_remote = docker::Engine::is_remote();
            let needs_docker = args
                .subcommand
                .map(|sc| sc.needs_docker(is_remote))
                .unwrap_or(false);
            if target.needs_docker() && needs_docker {
                let engine = docker::Engine::new(Some(is_remote), args.msg_info)?;
                if host_version_meta.needs_interpreter()
                    && needs_interpreter
                    && target.needs_interpreter()
                    && !interpreter::is_registered(&target)?
                {
                    docker::register(&engine, &target, args.msg_info)?
                }

                let status = docker::run(
                    &engine,
                    &target,
                    &filtered_args,
                    &metadata,
                    &config,
                    uses_xargo,
                    &sysroot,
                    args.msg_info,
                    args.docker_in_docker,
                    &cwd,
                )
                .wrap_err("could not run container")?;
                let needs_host = args
                    .subcommand
                    .map(|sc| sc.needs_host(is_remote))
                    .unwrap_or(false);
                if !status.success() {
                    warn_on_failure(&target, &toolchain, args.msg_info)?;
                }
                if !(status.success() && needs_host) {
                    return Ok(status);
                }
            }
        }
    }

    // if we fallback to the host cargo, use the same invocation that was made to cross
    let argv: Vec<String> = env::args().skip(1).collect();
    shell::note("Falling back to `cargo` on the host.", args.msg_info)?;
    match args.subcommand {
        Some(Subcommand::List) => {
            // this won't print in order if we have both stdout and stderr.
            let out = cargo::run_and_get_output(&argv, args.msg_info)?;
            let stdout = out.stdout()?;
            if out.status.success() && cli::is_subcommand_list(&stdout) {
                cli::fmt_subcommands(&stdout, args.msg_info)?;
            } else {
                // Not a list subcommand, which can happen with weird edge-cases.
                print!("{}", stdout);
                io::stdout().flush().unwrap();
            }
            Ok(out.status)
        }
        _ => cargo::run(&argv, args.msg_info).map_err(Into::into),
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
    msg_info: MessageInfo,
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
            shell::warn(format!("using older {rustc_warning}.\n > Update with `rustup update --force-non-host {toolchain}`"), msg_info)?;
            return Ok(VersionMatch::OlderTarget);
        } else if versions.is_gt() || (versions.is_eq() && dates.is_gt()) {
            shell::warn(
                format!("using newer {rustc_warning}.\n > Update with `rustup update`"),
                msg_info,
            )?;
            return Ok(VersionMatch::NewerTarget);
        } else {
            shell::warn(format!("using {rustc_warning}."), msg_info)?;
            return Ok(VersionMatch::Different);
        }
    }
    Ok(VersionMatch::Same)
}

/// Obtains the [`CrossToml`] from one of the possible locations
///
/// These locations are checked in the following order:
/// 1. If the `CROSS_CONFIG` variable is set, it tries to read the config from its value
/// 2. Otherwise, the `Cross.toml` in the project root is used
/// 3. Package metadata in the Cargo.toml
///
/// The values from `CROSS_CONFIG` or `Cross.toml` are concatenated with the package
/// metadata in `Cargo.toml`, with `Cross.toml` having the highest priority.
fn toml(metadata: &CargoMetadata, msg_info: MessageInfo) -> Result<Option<CrossToml>> {
    let root = &metadata.workspace_root;
    let cross_config_path = match env::var("CROSS_CONFIG") {
        Ok(var) => PathBuf::from(var),
        Err(_) => root.join("Cross.toml"),
    };

    // Attempts to read the cross config from the Cargo.toml
    let cargo_toml_str =
        file::read(root.join("Cargo.toml")).wrap_err("failed to read Cargo.toml")?;

    if cross_config_path.exists() {
        let cross_toml_str = file::read(&cross_config_path)
            .wrap_err_with(|| format!("could not read file `{cross_config_path:?}`"))?;

        let (config, _) = CrossToml::parse(&cargo_toml_str, &cross_toml_str, msg_info)
            .wrap_err_with(|| format!("failed to parse file `{cross_config_path:?}` as TOML",))?;

        Ok(Some(config))
    } else {
        // Checks if there is a lowercase version of this file
        if root.join("cross.toml").exists() {
            shell::warn("There's a file named cross.toml, instead of Cross.toml. You may want to rename it, or it won't be considered.", msg_info)?;
        }

        if let Some((cfg, _)) = CrossToml::parse_from_cargo(&cargo_toml_str, msg_info)? {
            Ok(Some(cfg))
        } else {
            Ok(None)
        }
    }
}
