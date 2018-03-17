use std::env;

use Target;
use cargo::Subcommand;

pub struct Args {
    pub all: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub target: Option<Target>,
    pub toolchain: Option<String>,
}

pub fn parse() -> Args {
    let mut all: Vec<_> = env::args().skip(1).collect();

    let mut target = None;
    let mut sc = None;
    let mut tc = None;

    // Attempt to find toolchain
    if all.len() > 0 && all[0].starts_with('+') && tc.is_none() {
        tc = Some(all[0][1..].to_owned());
        all.remove(0);
    }

    {
        let mut args = all.iter();
        while let Some(arg) = args.next() {
            if !arg.starts_with('-') && sc.is_none() {
                sc = Some(Subcommand::from(&**arg));
                continue;
            }

            if arg == "--target" {
                target = args.next().map(|s| Target::from(&**s))
            } else if arg.starts_with("--target=") {
                target = arg.splitn(2, '=')
                    .nth(1)
                    .map(|s| Target::from(&*s))
            }
        }
    }

    Args {
        all: all,
        subcommand: sc,
        target: target,
        toolchain: tc,
    }
}
