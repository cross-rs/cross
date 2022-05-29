use std::path::Path;

use crate::errors::*;
use crate::file;
use crate::Target;

/// Checks if the interpreters have been registered in the host system
pub fn is_registered(target: &Target) -> Result<bool> {
    if file::read("/proc/sys/fs/binfmt_misc/status")?.trim() != "enabled" {
        eyre::bail!("host system doesn't have binfmt_misc support")
    }

    let ok = if target.is_windows() {
        let wine = Path::new("/proc/sys/fs/binfmt_misc/wine");
        wine.exists() && {
            let f = file::read(wine)?;
            f.contains("/usr/bin/run-detectors")
                || f.contains("/usr/lib/binfmt-support/run-detectors")
        }
    } else {
        // NOTE checking any architecture will do, here we pick arm
        let qemu = Path::new("/proc/sys/fs/binfmt_misc/qemu-arm");
        qemu.exists() && file::read(qemu)?.contains("/usr/bin/qemu-arm-static")
    };

    Ok(ok)
}
