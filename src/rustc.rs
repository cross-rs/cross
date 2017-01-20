use std::path::PathBuf;
use std::process::Command;

use rustc_version;

use Host;
use errors::*;
use extensions::CommandExt;

pub struct TargetList {
    triples: Vec<String>,
}

impl TargetList {
    pub fn contains(&self, triple: &str) -> bool {
        self.triples.iter().any(|t| t == triple)
    }
}

pub fn host() -> Host {
    Host::from(&*rustc_version::version_meta().host)
}

pub fn target_list(verbose: bool) -> Result<TargetList> {
    Command::new("rustc")
        .args(&["--print", "target-list"])
        .run_and_get_stdout(verbose)
        .map(|s| {
            TargetList { triples: s.lines().map(|l| l.to_owned()).collect() }
        })
}

pub fn sysroot(verbose: bool) -> Result<PathBuf> {
    let mut stdout = Command::new("rustc").args(&["--print", "sysroot"])
        .run_and_get_stdout(verbose)?;

    if stdout.ends_with('\n') {
        stdout.pop();
    }

    Ok(PathBuf::from(stdout))
}
