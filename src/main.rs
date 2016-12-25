#[macro_use]
extern crate error_chain;
extern crate libc;
extern crate rustc_version;

mod cargo;
mod cli;
mod docker;
mod errors;
mod extensions;
mod id;
mod rustc;
mod rustup;

use std::io::Write;
use std::process::ExitStatus;
use std::{env, io, process};

use errors::*;

// Supported targets
const TARGETS: [&'static str; 12] = ["aarch64-unknown-linux-gnu",
                                     "armv7-unknown-linux-gnueabihf",
                                     "i686-unknown-linux-gnu",
                                     "mips-unknown-linux-gnu",
                                     "mips64-unknown-linux-gnuabi64",
                                     "mips64el-unknown-linux-gnuabi64",
                                     "mipsel-unknown-linux-gnu",
                                     "powerpc-unknown-linux-gnu",
                                     "powerpc64-unknown-linux-gnu",
                                     "powerpc64le-unknown-linux-gnu",
                                     "s390x-unknown-linux-gnu",
                                     "x86_64-unknown-linux-gnu"];

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

    match args.subcommand.as_ref().map(|s| &**s) {
        Some("build") | Some("run") | Some("rustc") | Some("test") => {
            let host = rustc::host();
            let supported = host == "x86_64-unknown-linux-gnu";
            let target = args.target.unwrap_or(host);

            match cargo::root()? {
                Some(ref root) if supported && TARGETS.contains(&&*target) => {
                    if !rustup::installed_targets()?.contains(&target) {
                        rustup::install(&target)?;
                    }

                    docker::run(&target, &args.all, &root)
                }
                _ => cargo::run(&args.all),
            }
        }
        _ => cargo::run(&args.all),
    }
}
