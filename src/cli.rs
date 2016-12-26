use std::env;

use Target;
use cargo::Subcommand;

pub struct Args {
    pub all: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub target: Option<Target>,
}

pub fn parse() -> Args {
    let all: Vec<_> = env::args().skip(1).collect();

    let mut target = None;
    let mut sc = None;

    {
        let mut args = all.iter();
        while let Some(arg) = args.next() {
            if !arg.starts_with("-") && sc.is_none() {
                sc = Some(Subcommand::from(&**arg))
            }

            if arg == "--target" {
                target = args.next().map(|s| Target::from(&**s))
            } else if arg.starts_with("--target=") {
                target = arg.splitn(2, '=')
                    .skip(1)
                    .next()
                    .map(|s| Target::from(&*s))
            } else if !arg.starts_with("-") && sc.is_none() {
                sc = Some(Subcommand::from(&**arg));
            }
        }
    }

    Args {
        all: all,
        subcommand: sc,
        target: target,
    }
}
