use std::env;

use crate::Target;
use crate::cargo::Subcommand;
use crate::rustc::TargetList;

pub struct Args {
    pub all: Vec<String>,
    pub subcommand: Option<Subcommand>,
    pub target: Option<Target>,
    pub target_dir: Option<String>,
}

pub fn parse(target_list: &TargetList) -> Args {
    let mut all: Vec<_> = env::args().skip(1).collect();

    let mut target = None;
    let mut target_dir = None;
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
            } else if arg == "--target-dir" {
                target_dir = args.next().map(|s| s.clone());
            } else if arg.starts_with("--target-dir=") {
                target_dir = arg.splitn(2, '=').nth(1).map(|s| s.to_owned())
            } else if !arg.starts_with('-') && sc.is_none() {
                sc = Some(Subcommand::from(&**arg));
            }
        }
    }

    // delete target-dir from args.all
    if let Some(ind) = all.iter().position(|x| x=="--target-dir") {
      all[ind]=String::new();
      all[ind+1]=String::new();
    }
    if let Some(ind) = all.iter().position(|x| x.starts_with("--target-dir=")) {
      all[ind]=String::new();
    }
    all = all.into_iter().filter(|x| !x.is_empty()).collect();

    Args {
        all: all,
        subcommand: sc,
        target: target,
        target_dir: target_dir,
    }
}
