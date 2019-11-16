use std::env;

use crate::Target;
use crate::cargo::Subcommand;
use crate::rustc::TargetList;

pub struct Args {
    pub all: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub target: Option<Target>,
}

pub fn parse(target_list: &TargetList) -> Args {
    let all: Vec<_> = env::args().skip(1).collect();

    let mut target = None;
    let mut sc = None;

    {
        let mut args = all.iter();
        while let Some(arg) = args.next() {
            if !arg.starts_with('-') && sc.is_none() {
                sc = Some(Subcommand::from(&**arg))
            }

            if arg == "--target" {
                target = args.next().map(|s| Target::from(&**s, target_list))
            } else if arg.starts_with("--target=") {
                target = arg.splitn(2, '=')
                    .nth(1)
                    .map(|s| Target::from(&*s, target_list))
            } else if !arg.starts_with('-') && sc.is_none() {
                sc = Some(Subcommand::from(&**arg));
            }
        }
    }

    Args {
        all,
        subcommand: sc,
        target,
    }
}
