use std::path::Path;

use errors::*;
use file;

/// Checks if the QEMU interpreters have been registered in the host system
pub fn is_registered() -> Result<bool> {
    if file::read("/proc/sys/fs/binfmt_misc/status")?.trim() != "enabled" {
        Err("host system doesn't have binfmt_misc support")?
    }

    // NOTE checking any architecture will do, here we pick arm
    let interpreter = Path::new("/proc/sys/fs/binfmt_misc/qemu-arm");

    Ok(interpreter.exists() &&
       file::read(interpreter)
        ?
        .contains("/usr/bin/qemu-arm-static"))
}
