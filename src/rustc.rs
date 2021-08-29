use std::path::PathBuf;
use std::process::Command;

use rustc_version::{Version, VersionMeta};

use crate::{Host, Target};
use crate::errors::*;
use crate::extensions::CommandExt;

pub struct TargetList {
    pub triples: Vec<String>,
}

impl TargetList {
    pub fn contains(&self, triple: &str) -> bool {
        self.triples.iter().any(|t| t == triple)
    }
}

pub trait VersionMetaExt {
    fn host(&self) -> Host;
    fn needs_interpreter(&self) -> bool;
}

impl VersionMetaExt for VersionMeta {
    fn host(&self) -> Host {
        Host::from(&*self.host)
    }

    fn needs_interpreter(&self) -> bool {
        self.semver < Version {
            major: 1,
            minor: 19,
            patch: 0,
            pre: vec![],
            build: vec![],
        }
    }
}

pub fn target_list(verbose: bool) -> Result<TargetList> {
    Command::new("rustc")
        .args(&["--print", "target-list"])
        .run_and_get_stdout(verbose)
        .map(|s| {
            TargetList {
                triples: s.lines().map(|l| l.to_owned()).collect(),
            }
        })
}

pub fn sysroot(host: &Host, target: &Target, verbose: bool) -> Result<PathBuf> {
    let mut stdout = Command::new("rustc")
        .args(&["--print", "sysroot"])
        .run_and_get_stdout(verbose)?;

    if stdout.ends_with('\n') {
        stdout.pop();
    }

    // On hosts other than Linux, specify the correct toolchain path.
    if host != &Host::X86_64UnknownLinuxGnu && target.needs_docker() {
        stdout = stdout.replacen(host.triple(), Host::X86_64UnknownLinuxGnu.triple(), 1);
    }

    Ok(PathBuf::from(stdout))
}
