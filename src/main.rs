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
    X86_64UnknownLinuxGnu,
    Other,
}

impl<'a> From<&'a str> for Host {
    fn from(s: &str) -> Host {
        match s {
            "x86_64-unknown-linux-gnu" => Host::X86_64UnknownLinuxGnu,
            _ => Host::Other,
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq)]
pub enum Target {
    Aarch64UnknownLinuxGnu,
    Armv7UnknownLinuxGnueabihf,
    I686UnknownLinuxGnu,
    Mips64UnknownLinuxGnuabi64,
    Mips64elUnknownLinuxGnuabi64,
    MipsUnknownLinuxGnu,
    MipselUnknownLinuxGnu,
    Other,
    Powerpc64UnknownLinuxGnu,
    Powerpc64leUnknownLinuxGnu,
    PowerpcUnknownLinuxGnu,
    S390xUnknownLinuxGnu,
    X86_64UnknownLinuxGnu,
}

impl Target {
    fn needs_docker(&self) -> bool {
        match *self {
            Target::Other => false,
            _ => true,
        }
    }

    fn needs_qemu(&self) -> bool {
        match *self {
            Target::I686UnknownLinuxGnu |
            Target::Other |
            Target::X86_64UnknownLinuxGnu => false,
            _ => true,
        }
    }

    fn triple(&self) -> &'static str {
        use Target::*;

        match *self {
            Aarch64UnknownLinuxGnu => "aarch64-unknown-linux-gnu",
            Armv7UnknownLinuxGnueabihf => "armv7-unknown-linux-gnueabihf",
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
            X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
        }
    }
}

impl<'a> From<&'a str> for Target {
    fn from(s: &str) -> Target {
        use Target::*;

        match s {
            "aarch64-unknown-linux-gnu" => Aarch64UnknownLinuxGnu,
            "armv7-unknown-linux-gnueabihf" => Armv7UnknownLinuxGnueabihf,
            "i686-unknown-linux-gnu" => I686UnknownLinuxGnu,
            "mips-unknown-linux-gnu" => MipsUnknownLinuxGnu,
            "mips64-unknown-linux-gnuabi64" => Mips64UnknownLinuxGnuabi64,
            "mips64el-unknown-linux-gnuabi64" => Mips64elUnknownLinuxGnuabi64,
            "mipsel-unknown-linux-gnu" => MipselUnknownLinuxGnu,
            "powerpc-unknown-linux-gnu" => PowerpcUnknownLinuxGnu,
            "powerpc64-unknown-linux-gnu" => Powerpc64UnknownLinuxGnu,
            "powerpc64le-unknown-linux-gnu" => Powerpc64leUnknownLinuxGnu,
            "s390x-unknown-linux-gnu" => S390xUnknownLinuxGnu,
            "x86_64-unknown-linux-gnu" => X86_64UnknownLinuxGnu,
            _ => Other,
        }
    }
}

impl From<Host> for Target {
    fn from(h: Host) -> Target {
        match h {
            Host::X86_64UnknownLinuxGnu => Target::X86_64UnknownLinuxGnu,
            _ => Target::Other,
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

    let host = rustc::host();

    if host == Host::X86_64UnknownLinuxGnu {
        let target = args.target.unwrap_or(Target::X86_64UnknownLinuxGnu);

        if target.needs_docker() &&
           args.subcommand.map(|sc| sc.needs_docker()).unwrap_or(false) {
            if let Some(root) = cargo::root()? {
                if !rustup::installed_targets()?.contains(&target) {
                    rustup::install(target)?;
                }

                if args.subcommand.map(|sc| sc.needs_qemu()).unwrap_or(false) &&
                   target.needs_qemu() &&
                   !qemu::is_registered()? {
                    docker::register()?
                }

                return docker::run(target, &args.all, &root);
            }
        }
    }

    cargo::run(&args.all)
}
