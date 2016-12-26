use std::path::PathBuf;
use std::process::Command;

use rustc_version;

use Host;
use errors::*;
use extensions::CommandExt;

pub fn host() -> Host {
    Host::from(&*rustc_version::version_meta().host)
}

pub fn sysroot() -> Result<PathBuf> {
    let mut stdout = Command::new("rustc").args(&["--print", "sysroot"])
        .run_and_get_stdout()?;

    if stdout.ends_with('\n') {
        stdout.pop();
    }

    Ok(PathBuf::from(stdout))
}
