#[macro_use]
extern crate error_chain;
extern crate libc;
extern crate rustc_version;

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

use errors::*;

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
    fn is_supported(&self, target: Option<Target>) -> bool {
        if *self == Host::X86_64AppleDarwin {
            target == Some(Target::I686AppleDarwin)
        } else if *self == Host::X86_64UnknownLinuxGnu {
            target.map(|t| t.is_linux()).unwrap_or(true)
        } else {
            false
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
#[derive(Clone, Copy, PartialEq)]
pub enum Target {
    Other,

    // OSX
    I686AppleDarwin,
    X86_64AppleDarwin,

    // Linux
    Aarch64UnknownLinuxGnu,
    Armv7UnknownLinuxGnueabihf,
    I686UnknownLinuxGnu,
    Mips64UnknownLinuxGnuabi64,
    Mips64elUnknownLinuxGnuabi64,
    MipsUnknownLinuxGnu,
    MipselUnknownLinuxGnu,
    Powerpc64UnknownLinuxGnu,
    Powerpc64leUnknownLinuxGnu,
    PowerpcUnknownLinuxGnu,
    S390xUnknownLinuxGnu,
    X86_64UnknownLinuxGnu,
    X86_64UnknownLinuxMusl,
}

impl Target {
    fn is_linux(&self) -> bool {
        match *self {
            Target::I686AppleDarwin |
            Target::Other |
            Target::X86_64AppleDarwin => false,
            _ => true,
        }
    }

    fn needs_docker(&self) -> bool {
        self.is_linux()
    }

    fn needs_qemu(&self) -> bool {
        self.is_linux() &&
        match *self {
            Target::I686UnknownLinuxGnu |
            Target::X86_64UnknownLinuxGnu |
            Target::X86_64UnknownLinuxMusl => false,
            _ => true,
        }
    }

    fn triple(&self) -> &'static str {
        use Target::*;

        match *self {
            Aarch64UnknownLinuxGnu => "aarch64-unknown-linux-gnu",
            Armv7UnknownLinuxGnueabihf => "armv7-unknown-linux-gnueabihf",
            I686AppleDarwin => "i686-apple-darwin",
            I686UnknownLinuxGnu => "i686-unknown-linux-gnu",
            Mips64UnknownLinuxGnuabi64 => "mips64-unknown-linux-gnuabi64",
            Mips64elUnknownLinuxGnuabi64 => "mips64el-unknown-linux-gnuabi64",
            MipsUnknownLinuxGnu => "mips-unknown-linux-gnu",
            MipselUnknownLinuxGnu => "mipsel-unknown-linux-gnu",
            Other => unreachable!(),
            Powerpc64UnknownLinuxGnu => "powerpc64-unknown-linux-gnu",
            Powerpc64leUnknownLinuxGnu => "powerpc64le-unknown-linux-gnu",
            PowerpcUnknownLinuxGnu => "powerpc-unknown-linux-gnu",
            S390xUnknownLinuxGnu => "s390x-unknown-linux-gnu",
            X86_64AppleDarwin => "x86_64-apple-darwin",
            X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            X86_64UnknownLinuxMusl => "x86_64-unknown-linux-musl",
        }
    }
}

impl<'a> From<&'a str> for Target {
    fn from(s: &str) -> Target {
        use Target::*;

        match s {
            "aarch64-unknown-linux-gnu" => Aarch64UnknownLinuxGnu,
            "armv7-unknown-linux-gnueabihf" => Armv7UnknownLinuxGnueabihf,
            "i686-apple-darwin" => I686AppleDarwin,
            "i686-unknown-linux-gnu" => I686UnknownLinuxGnu,
            "mips-unknown-linux-gnu" => MipsUnknownLinuxGnu,
            "mips64-unknown-linux-gnuabi64" => Mips64UnknownLinuxGnuabi64,
            "mips64el-unknown-linux-gnuabi64" => Mips64elUnknownLinuxGnuabi64,
            "mipsel-unknown-linux-gnu" => MipselUnknownLinuxGnu,
            "powerpc-unknown-linux-gnu" => PowerpcUnknownLinuxGnu,
            "powerpc64-unknown-linux-gnu" => Powerpc64UnknownLinuxGnu,
            "powerpc64le-unknown-linux-gnu" => Powerpc64leUnknownLinuxGnu,
            "s390x-unknown-linux-gnu" => S390xUnknownLinuxGnu,
            "x86_64-apple-darwin" => X86_64AppleDarwin,
            "x86_64-unknown-linux-gnu" => X86_64UnknownLinuxGnu,
            "x86_64-unknown-linux-musl" => X86_64UnknownLinuxMusl,
            _ => Other,
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

fn main() {
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
    let args = cli::parse();

    if args.all.iter().any(|a| a == "--version" || a == "-V") {
        println!(concat!("cross ", env!("CARGO_PKG_VERSION"), "{}"),
                 include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt")));
    }

    let verbose =
        args.all.iter().any(|a| a == "--verbose" || a == "-v" || a == "-vv");

    if let Some(root) = cargo::root()? {
        let host = rustc::host();

        if host.is_supported(args.target) {
            let target = args.target.unwrap_or(Target::from(host));

            if !rustup::installed_targets(verbose)?.contains(&target) {
                rustup::install(target, verbose)?;
            }

            if target.needs_docker() &&
               args.subcommand.map(|sc| sc.needs_docker()).unwrap_or(false) {
                if args.subcommand.map(|sc| sc.needs_qemu()).unwrap_or(false) &&
                   target.needs_qemu() &&
                   !qemu::is_registered()? {
                    docker::register(verbose)?
                }

                return docker::run(target, &args.all, &root, verbose);
            }
        }
    }

    cargo::run(&args.all, verbose)
}
