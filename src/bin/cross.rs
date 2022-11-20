#![deny(missing_debug_implementations, rust_2018_idioms)]

use std::env;
use std::io::{self, Write};
use std::process::ExitStatus;

use cross::shell::{MessageInfo, Verbosity};
use cross::{cargo, cli, rustc, OutputExt, Subcommand};

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    cross::install_termination_hook()?;

    let target_list = rustc::target_list(&mut Verbosity::Quiet.into())?;
    let args = cli::parse(&target_list)?;
    let subcommand = args.subcommand;
    // subcommands like `--version` ignore `--quiet`.
    let quiet = match subcommand
        .map(|x| !x.is_flag_subcommand())
        .unwrap_or_default()
    {
        true => args.quiet,
        false => false,
    };
    let mut msg_info = MessageInfo::create(args.verbose, quiet, args.color.as_deref())?;
    // short-circuit to avoid external calls unless necessary
    let status = match subcommand {
        Some(Subcommand::Version) => {
            msg_info.print(cross::version())?;
            None
        }
        Some(sc) if sc.never_needs_docker() => None,
        None => None,
        Some(_) => cross::run(args, target_list, &mut msg_info)?,
    };
    let status = match status {
        Some(status) => status,
        None => {
            // if we fallback to the host cargo, use the same invocation that was made to cross
            let argv: Vec<String> = env::args().skip(1).collect();
            msg_info.note("Falling back to `cargo` on the host.")?;
            match subcommand {
                Some(Subcommand::List) => fmt_list(&argv, &mut msg_info)?,
                _ => cargo::run(&argv, &mut msg_info)?,
            }
        }
    };
    let code = status
        .code()
        .ok_or_else(|| eyre::Report::msg("Cargo process terminated by signal"))?;
    std::process::exit(code)
}

fn fmt_list(args: &[String], msg_info: &mut MessageInfo) -> cross::Result<ExitStatus> {
    // this won't print in order if we have both stdout and stderr.
    let out = cargo::run_and_get_output(args, msg_info)?;
    let stdout = out.stdout()?;
    if out.status.success() && cli::is_subcommand_list(&stdout) {
        cli::fmt_subcommands(&stdout, msg_info)?;
    } else {
        // Not a list subcommand, which can happen with weird edge-cases.
        print!("{}", stdout);
        io::stdout().flush().expect("could not flush");
    }
    Ok(out.status)
}
