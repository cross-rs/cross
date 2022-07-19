#![cfg(target_family = "unix")]
#![doc(hidden)]

use std::env;
use std::fs::File;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use crate::{Error, Result};

#[derive(Debug)]
pub struct InnerLock {
    file: File,
}

impl InnerLock {
    pub fn create(name: impl AsRef<str>) -> Result<InnerLock> {
        match is_valid_filename(name.as_ref()) {
            true => {
                let mut file_name = name.as_ref().to_owned();
                file_name.push_str(".lock");
                InnerLock::with_path(env::temp_dir().join(file_name))
            }
            false => Err(Error::InvalidCharacter),
        }
    }

    pub fn with_path(path: impl AsRef<Path>) -> Result<InnerLock> {
        Ok(InnerLock {
            file: File::create(path).map_err(Error::CreateFailed)?,
        })
    }

    pub fn lock(&self) -> Result<InnerLockGuard<'_>> {
        // SAFETY: safe, since we have valid flags.
        unsafe { self._lock(libc::LOCK_EX) }
    }

    pub fn try_lock(&self) -> Result<InnerLockGuard<'_>> {
        // SAFETY: safe, since we have valid flags.
        unsafe { self._lock(libc::LOCK_EX | libc::LOCK_NB) }
    }

    /// # SAFETY
    ///
    /// Safe as long as the flags are correct.
    unsafe fn _lock(&self, flags: libc::c_int) -> Result<InnerLockGuard<'_>> {
        // SAFETY: safe if we have a valid file descriptor.
        let fd = self.file.as_raw_fd();
        match libc::flock(fd, flags) {
            0 => Ok(InnerLockGuard { lock: self }),
            libc::EWOULDBLOCK => Err(Error::WouldBlock),
            code => Err(Error::LockFailed(io::Error::from_raw_os_error(code))),
        }
    }

    pub fn unlock(&self) -> Result<()> {
        // SAFETY: safe, since we use valid flags
        unsafe { self._unlock(libc::LOCK_UN) }
    }

    /// # SAFETY
    ///
    /// Safe as long as the flags are correct.
    unsafe fn _unlock(&self, flags: libc::c_int) -> Result<()> {
        let fd = self.file.as_raw_fd();
        match libc::flock(fd, flags) {
            0 => Ok(()),
            code => Err(Error::UnlockFailed(io::Error::from_raw_os_error(code))),
        }
    }
}

#[derive(Debug)]
pub struct InnerLockGuard<'a> {
    lock: &'a InnerLock,
}

impl<'a> Drop for InnerLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock().ok();
    }
}

fn is_valid_filename(name: &str) -> bool {
    // the name cannot have a /
    !name.contains('/')
}
