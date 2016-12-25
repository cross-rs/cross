use std::env;

pub struct Args {
    pub all: Vec<String>,
    pub subcommand: Option<String>,
    pub target: Option<String>,
}

pub fn parse() -> Args {
    let all: Vec<_> = env::args().skip(1).collect();

    let mut target = None;
    let mut sc = None;

    {
        let mut args = all.iter();
        while let Some(arg) = args.next() {
            if !arg.starts_with("-") && sc.is_none() {
                sc = Some(arg.clone())
            }

            if arg == "--target" {
                target = args.next().cloned()
            } else if arg.starts_with("--target=") {
                target = arg.splitn(2, '=').skip(1).next().map(|s| s.to_owned())
            } else if !arg.starts_with("-") && sc.is_none() {
                sc = Some(arg.clone());
            }
        }
    }

    Args {
        all: all,
        subcommand: sc,
        target: target,
    }
}
