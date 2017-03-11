#[macro_use]
extern crate error_chain;
extern crate libc;
extern crate rustc_version;
extern crate toml;

mod cargo;
mod cli;
mod docker;
mod errors;
mod extensions;
mod file;
mod id;
mod qemu;
mod rustc;
mod rustup;

use std::io::Write;
use std::process::ExitStatus;
use std::{env, io, process};

use toml::{Parser, Value};

use cargo::Root;
use errors::*;
use rustc::TargetList;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq)]
pub enum Host {
    Other,

    // OSX
    X86_64AppleDarwin,

    // Linux
    X86_64UnknownLinuxGnu,
}

impl Host {
    /// Checks if this `(host, target)` pair is supported by `cross`
    ///
    /// `target == None` means `target == host`
    fn is_supported(&self, target: Option<&Target>) -> bool {
        if *self == Host::X86_64AppleDarwin {
            target.map(|t| *t == Target::I686AppleDarwin).unwrap_or(false)
        } else if *self == Host::X86_64UnknownLinuxGnu {
            target.map(|t| t.needs_docker()).unwrap_or(true)
        } else {
            false
        }
    }

    fn triple(&self) -> &'static str {
        match *self {
            Host::X86_64AppleDarwin => "x86_64-apple-darwin",
            Host::X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            Host::Other => unimplemented!(),
        }
    }
}

impl<'a> From<&'a str> for Host {
    fn from(s: &str) -> Host {
        match s {
            "x86_64-apple-darwin" => Host::X86_64AppleDarwin,
            "x86_64-unknown-linux-gnu" => Host::X86_64UnknownLinuxGnu,
            _ => Host::Other,
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, PartialEq)]
pub enum Target {
    Custom { triple: String },

    // Other built-in
    Other,

    // OSX
    I686AppleDarwin,
    X86_64AppleDarwin,

    // Linux
    Aarch64UnknownLinuxGnu,
    ArmUnknownLinuxGnueabi,
    ArmUnknownLinuxMusleabi,
    Armv7UnknownLinuxGnueabihf,
    Armv7UnknownLinuxMusleabihf,
    I686UnknownLinuxGnu,
    I686UnknownLinuxMusl,
    Mips64UnknownLinuxGnuabi64,
    Mips64elUnknownLinuxGnuabi64,
    MipsUnknownLinuxGnu,
    MipselUnknownLinuxGnu,
    Powerpc64UnknownLinuxGnu,
    Powerpc64leUnknownLinuxGnu,
    PowerpcUnknownLinuxGnu,
    S390xUnknownLinuxGnu,
    Sparc64UnknownLinuxGnu,
    X86_64UnknownLinuxGnu,
    X86_64UnknownLinuxMusl,

    // *BSD
    I686UnknownFreebsd,
    X86_64UnknownDragonfly,
    X86_64UnknownFreebsd,
    X86_64UnknownNetbsd,

    // Windows
    X86_64PcWindowsGnu,

    // Bare metal
    Thumbv6mNoneEabi,
    Thumbv7emNoneEabi,
    Thumbv7emNoneEabihf,
    Thumbv7mNoneEabi,
}

impl Target {
    fn is_bare_metal(&self) -> bool {
        match *self {
            Target::Thumbv6mNoneEabi |
            Target::Thumbv7emNoneEabi |
            Target::Thumbv7emNoneEabihf |
            Target::Thumbv7mNoneEabi => true,
            _ => false,
        }
    }

    fn is_builtin(&self) -> bool {
        match *self {
            Target::Custom { .. } => false,
            _ => true,
        }
    }

    fn is_bsd(&self) -> bool {
        match *self {
            Target::I686UnknownFreebsd |
            Target::X86_64UnknownDragonfly |
            Target::X86_64UnknownFreebsd |
            Target::X86_64UnknownNetbsd => true,
            _ => false,
        }
    }

    fn is_linux(&self) -> bool {
        match *self {
            Target::Aarch64UnknownLinuxGnu |
            Target::ArmUnknownLinuxGnueabi |
            Target::ArmUnknownLinuxMusleabi |
            Target::Armv7UnknownLinuxGnueabihf |
            Target::Armv7UnknownLinuxMusleabihf |
            Target::I686UnknownLinuxGnu |
            Target::I686UnknownLinuxMusl |
            Target::Mips64UnknownLinuxGnuabi64 |
            Target::Mips64elUnknownLinuxGnuabi64 |
            Target::MipsUnknownLinuxGnu |
            Target::MipselUnknownLinuxGnu |
            Target::Powerpc64UnknownLinuxGnu |
            Target::Powerpc64leUnknownLinuxGnu |
            Target::PowerpcUnknownLinuxGnu |
            Target::S390xUnknownLinuxGnu |
            Target::Sparc64UnknownLinuxGnu |
            Target::X86_64UnknownLinuxGnu |
            Target::X86_64UnknownLinuxMusl => true,
            _ => false,
        }
    }

    fn is_windows(&self) -> bool {
        match *self {
            Target::X86_64PcWindowsGnu => true,
            _ => false,
        }
    }

    fn needs_docker(&self) -> bool {
        self.is_linux() || self.is_bare_metal() || self.is_bsd() ||
        !self.is_builtin() || self.is_windows()
    }

    fn needs_qemu(&self) -> bool {
        let not_native = match *self {
            Target::Custom { ref triple } => {
                return !triple.starts_with("x86_64") &&
                       !triple.starts_with("i586") &&
                       !triple.starts_with("i686")
            }
            Target::I686UnknownLinuxGnu |
            Target::I686UnknownLinuxMusl |
            Target::X86_64UnknownLinuxGnu |
            Target::X86_64UnknownLinuxMusl => false,
            _ => true,
        };

        not_native && (self.is_linux() || self.is_bare_metal())
    }

    fn triple(&self) -> &str {
        use Target::*;

        match *self {
            Custom { ref triple } => triple,
            Other => unreachable!(),

            Aarch64UnknownLinuxGnu => "aarch64-unknown-linux-gnu",
            ArmUnknownLinuxGnueabi => "arm-unknown-linux-gnueabi",
            ArmUnknownLinuxMusleabi => "arm-unknown-linux-musleabi",
            Armv7UnknownLinuxGnueabihf => "armv7-unknown-linux-gnueabihf",
            Armv7UnknownLinuxMusleabihf => "armv7-unknown-linux-musleabihf",
            I686AppleDarwin => "i686-apple-darwin",
            I686UnknownFreebsd => "i686-unknown-freebsd",
            I686UnknownLinuxGnu => "i686-unknown-linux-gnu",
            I686UnknownLinuxMusl => "i686-unknown-linux-musl",
            Mips64UnknownLinuxGnuabi64 => "mips64-unknown-linux-gnuabi64",
            Mips64elUnknownLinuxGnuabi64 => "mips64el-unknown-linux-gnuabi64",
            MipsUnknownLinuxGnu => "mips-unknown-linux-gnu",
            MipselUnknownLinuxGnu => "mipsel-unknown-linux-gnu",
            Powerpc64UnknownLinuxGnu => "powerpc64-unknown-linux-gnu",
            Powerpc64leUnknownLinuxGnu => "powerpc64le-unknown-linux-gnu",
            PowerpcUnknownLinuxGnu => "powerpc-unknown-linux-gnu",
            S390xUnknownLinuxGnu => "s390x-unknown-linux-gnu",
            Sparc64UnknownLinuxGnu => "sparc64-unknown-linux-gnu",
            Thumbv6mNoneEabi => "thumbv6m-none-eabi",
            Thumbv7emNoneEabi => "thumbv7em-none-eabi",
            Thumbv7emNoneEabihf => "thumbv7em-none-eabihf",
            Thumbv7mNoneEabi => "thumbv7m-none-eabi",
            X86_64AppleDarwin => "x86_64-apple-darwin",
            X86_64PcWindowsGnu => "x86_64-pc-windows-gnu",
            X86_64UnknownDragonfly => "x86_64-unknown-dragonfly",
            X86_64UnknownFreebsd => "x86_64-unknown-freebsd",
            X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            X86_64UnknownLinuxMusl => "x86_64-unknown-linux-musl",
            X86_64UnknownNetbsd => "x86_64-unknown-netbsd",
        }
    }

    fn needs_xargo(&self) -> bool {
        self.is_bare_metal() || !self.is_builtin()
    }
}

impl Target {
    fn from(triple: &str, target_list: &TargetList) -> Target {
        use Target::*;

        match triple {
            "aarch64-unknown-linux-gnu" => Aarch64UnknownLinuxGnu,
            "arm-unknown-linux-gnueabi" => ArmUnknownLinuxGnueabi,
            "arm-unknown-linux-musleabi" => ArmUnknownLinuxMusleabi,
            "armv7-unknown-linux-gnueabihf" => Armv7UnknownLinuxGnueabihf,
            "armv7-unknown-linux-musleabihf" => Armv7UnknownLinuxMusleabihf,
            "i686-apple-darwin" => I686AppleDarwin,
            "i686-unknown-freebsd" => I686UnknownFreebsd,
            "i686-unknown-linux-gnu" => I686UnknownLinuxGnu,
            "i686-unknown-linux-musl" => I686UnknownLinuxMusl,
            "mips-unknown-linux-gnu" => MipsUnknownLinuxGnu,
            "mips64-unknown-linux-gnuabi64" => Mips64UnknownLinuxGnuabi64,
            "mips64el-unknown-linux-gnuabi64" => Mips64elUnknownLinuxGnuabi64,
            "mipsel-unknown-linux-gnu" => MipselUnknownLinuxGnu,
            "powerpc-unknown-linux-gnu" => PowerpcUnknownLinuxGnu,
            "powerpc64-unknown-linux-gnu" => Powerpc64UnknownLinuxGnu,
            "powerpc64le-unknown-linux-gnu" => Powerpc64leUnknownLinuxGnu,
            "s390x-unknown-linux-gnu" => S390xUnknownLinuxGnu,
            "sparc64-unknown-linux-gnu" => Sparc64UnknownLinuxGnu,
            "thumbv6m-none-eabi" => Thumbv6mNoneEabi,
            "thumbv7em-none-eabi" => Thumbv7emNoneEabi,
            "thumbv7em-none-eabihf" => Thumbv7emNoneEabihf,
            "thumbv7m-none-eabi" => Thumbv7mNoneEabi,
            "x86_64-apple-darwin" => X86_64AppleDarwin,
            "x86_64-pc-windows-gnu" => X86_64PcWindowsGnu,
            "x86_64-unknown-dragonfly" => X86_64UnknownDragonfly,
            "x86_64-unknown-freebsd" => X86_64UnknownFreebsd,
            "x86_64-unknown-linux-gnu" => X86_64UnknownLinuxGnu,
            "x86_64-unknown-linux-musl" => X86_64UnknownLinuxMusl,
            "x86_64-unknown-netbsd" => X86_64UnknownNetbsd,
            _ if target_list.contains(triple) => Other,
            _ => Custom { triple: triple.to_owned() },
        }
    }
}

impl From<Host> for Target {
    fn from(host: Host) -> Target {
        match host {
            Host::X86_64UnknownLinuxGnu => Target::X86_64UnknownLinuxGnu,
            Host::X86_64AppleDarwin => Target::X86_64AppleDarwin,
            Host::Other => unreachable!(),
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
                writeln!(stderr,
                         "note: run with `RUST_BACKTRACE=1` for a backtrace")
                    .ok();
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

    if args.all.iter().any(|a| a == "--version" || a == "-V") &&
       args.subcommand.is_none() {
        println!(concat!("cross ", env!("CARGO_PKG_VERSION"), "{}"),
                 include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt")));
    }

    let verbose =
        args.all.iter().any(|a| a == "--verbose" || a == "-v" || a == "-vv");

    if let Some(root) = cargo::root()? {
        let host = rustc::host();

        if host.is_supported(args.target.as_ref()) {
            let target = args.target
                .unwrap_or(Target::from(host.triple(), &target_list));
            let toml = toml(&root)?;
            let uses_xargo = if let Some(toml) = toml.as_ref() {
                    toml.xargo(&target)?
                } else {
                    None
                }
                .unwrap_or_else(|| target.needs_xargo());

            if !uses_xargo &&
               rustup::available_targets(verbose)?.contains(&target) {
                rustup::install(&target, verbose)?;
            }

            if uses_xargo && !rustup::rust_src_is_installed(verbose)? {
                rustup::install_rust_src(verbose)?;
            }

            if target.needs_docker() &&
               args.subcommand.map(|sc| sc.needs_docker()).unwrap_or(false) {
                if args.subcommand.map(|sc| sc.needs_qemu()).unwrap_or(false) &&
                   target.needs_qemu() &&
                   !qemu::is_registered()? {
                    docker::register(verbose)?
                }

                return docker::run(&target,
                                   &args.all,
                                   &root,
                                   toml.as_ref(),
                                   uses_xargo,
                                   verbose);
            }
        }
    }

    cargo::run(&args.all, verbose)
}

/// Parsed `Cross.toml`
pub struct Toml {
    table: Value,
}

impl Toml {
    /// Returns the `target.{}.image` part of `Cross.toml`
    pub fn image(&self, target: &Target) -> Result<Option<&str>> {
        let triple = target.triple();

        if let Some(value) = self.table
            .lookup(&format!("target.{}.image", triple)) {
            Ok(Some(value.as_str()
                .ok_or_else(|| {
                    format!("target.{}.image must be a string", triple)
                })?))
        } else {
            Ok(None)
        }
    }

    /// Returns the `build.image` or the `target.{}.xargo` part of `Cross.toml`
    pub fn xargo(&self, target: &Target) -> Result<Option<bool>> {
        let triple = target.triple();

        if let Some(value) = self.table.lookup("build.xargo") {
            return Ok(Some(value.as_bool()
                .ok_or_else(|| "build.xargo must be a boolean")?));
        }

        if let Some(value) = self.table
            .lookup(&format!("target.{}.xargo", triple)) {
            Ok(Some(value.as_bool()
                .ok_or_else(|| {
                    format!("target.{}.xargo must be a boolean", triple)
                })?))
        } else {
            Ok(None)
        }
    }

    /// Returns the `build.env.whitelist` part of `Cross.toml`
    pub fn env_whitelist(&self) -> Result<Vec<&str>> {
        if let Some(value) = self.table.lookup("build.env.whitelist") {
            if let Some(arr) = value.as_slice() {
                arr.iter()
                    .map(|v| {
                        v.as_str()
                            .ok_or_else(|| {
                                "every build.env.whitelist element must be a string".into()
                            })
                    })
                    .collect()
            } else {
                Err("build.env.whitelist must be an array".into())
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Returns the `build.env.whitelist_all` part of `Cross.toml`
    pub fn env_whitelist_all(&self) -> Result<Option<bool>> {
        if let Some(value) = self.table.lookup("build.env.whitelist_all") {
            value.as_bool()
                .ok_or_else(|| "build.env.whitelist_all must be a boolean".into())
                .map(Some)
        } else {
            Ok(None)
        }
    }
}

/// Parses the `Cross.toml` at the root of the Cargo project (if any)
fn toml(root: &Root) -> Result<Option<Toml>> {
    let path = root.path().join("Cross.toml");

    if path.exists() {
        Ok(Some(Toml {
            table: Value::Table(Parser::new(&file::read(&path)?).parse()
                .ok_or_else(|| {
                    format!("couldn't parse {} as TOML", path.display())
                })?),
        }))
    } else {
        Ok(None)
    }
}
