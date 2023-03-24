#![deny(missing_debug_implementations, rust_2018_idioms)]

use std::{
    env,
    io::{self, Write},
};

use cross::{
    cargo, cli, rustc,
    shell::{self, Verbosity},
    OutputExt, Subcommand,
};

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    cross::install_termination_hook()?;

    let target_list = rustc::target_list(&mut Verbosity::Quiet.into())?;
    let args = cli::parse(&target_list)?;
    let subcommand = args.subcommand.clone();
    let mut msg_info = shell::MessageInfo::create(args.verbose, args.quiet, args.color.as_deref())?;
    let status = match cross::run(args, target_list, &mut msg_info)? {
        Some(status) => status,
        None if !msg_info.should_fail() => {
            // if we fallback to the host cargo, use the same invocation that was made to cross
            let argv: Vec<String> = env::args().skip(1).collect();
            msg_info.note("Falling back to `cargo` on the host.")?;
            match subcommand {
                Some(Subcommand::List) => {
                    // this won't print in order if we have both stdout and stderr.
                    let out = cargo::run_and_get_output(&argv, &mut msg_info)?;
                    let stdout = out.stdout()?;
                    if out.status.success() && cli::is_subcommand_list(&stdout) {
                        cli::fmt_subcommands(&stdout, &mut msg_info)?;
                    } else {
                        // Not a list subcommand, which can happen with weird edge-cases.
                        print!("{}", stdout);
                        io::stdout().flush().expect("could not flush");
                    }
                    out.status
                }
                _ => cargo::run(&argv, &mut msg_info)?,
            }
        }
        None => {
            msg_info.error("Errors encountered before cross compilation, aborting.")?;
            msg_info.note("Disable this with `CROSS_NO_WARNINGS=0`")?;
            std::process::exit(1);
        }
    };
    let code = status
        .code()
        .ok_or_else(|| eyre::Report::msg("Cargo process terminated by signal"))?;
    std::process::exit(code)
}
