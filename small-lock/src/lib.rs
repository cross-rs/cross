//! A minimal-dependency, safe API for a named process lock.
//!
//! This uses `flock` on Unix-like systems and `CreateMutex` on Linux.
//! This is meant to be a drop-in-replacement for `named_lock`, and
//! therefore features an identical API.

#![deny(missing_debug_implementations, rust_2018_idioms)]
#![warn(
    clippy::explicit_into_iter_loop,
    clippy::explicit_iter_loop,
    clippy::implicit_clone,
    clippy::inefficient_to_string,
    clippy::map_err_ignore,
    clippy::map_unwrap_or,
    clippy::ref_binding_to_reference,
    clippy::semicolon_if_nothing_returned,
    clippy::str_to_string,
    clippy::string_to_string,
    // needs clippy 1.61 clippy::unwrap_used
)]

#[cfg(target_family = "unix")]
mod unix;
#[cfg(target_family = "unix")]
pub use unix::*;

#[cfg(target_family = "windows")]
mod windows;
#[cfg(target_family = "windows")]
pub use windows::*;

use std::error;
use std::fmt;
use std::io;
use std::result;

#[cfg(target_family = "unix")]
use std::path::Path;

/// Named (or by path) lock.
#[derive(Debug)]
pub struct NamedLock {
    inner: InnerLock,
}

impl NamedLock {
    /// Create a lock file with a given name.
    ///
    /// By default, this will the current temporary directory
    /// and create a file at `$TMP/{name}.lock`.
    pub fn create(name: impl AsRef<str>) -> Result<NamedLock> {
        Ok(NamedLock {
            inner: InnerLock::create(name)?,
        })
    }

    /// Creates a lock file at a specific path.
    /// The parent directory must exist.
    #[cfg(target_family = "unix")]
    pub fn with_path(path: impl AsRef<Path>) -> Result<NamedLock> {
        Ok(NamedLock {
            inner: InnerLock::with_path(path)?,
        })
    }

    /// Lock the named lock.
    pub fn lock(&self) -> Result<NamedLockGuard<'_>> {
        Ok(NamedLockGuard {
            _inner: self.inner.lock()?,
        })
    }

    /// Try to lock the named lock.
    ///
    /// Returns `Error::WouldBlock` if it cannot acquire the lock
    /// because it would block.
    pub fn try_lock(&self) -> Result<NamedLockGuard<'_>> {
        Ok(NamedLockGuard {
            _inner: self.inner.try_lock()?,
        })
    }

    /// Unlock the named lock.
    pub fn unlock(&self) -> Result<()> {
        self.inner.unlock()
    }
}

/// Scoped guard for the named lock.
#[derive(Debug)]
pub struct NamedLockGuard<'a> {
    _inner: InnerLockGuard<'a>,
}

/// Error type for the lockfile.
#[derive(Debug)]
pub enum Error {
    InvalidCharacter,
    CreateFailed(io::Error),
    LockFailed(io::Error),
    UnlockFailed(io::Error),
    WouldBlock,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCharacter => f.write_str("invalid character found in file name"),
            Error::CreateFailed(e) => f.write_fmt(format_args!("lock creation failed with {e}")),
            Error::LockFailed(e) => f.write_fmt(format_args!("unable to acquire lock with {e}")),
            Error::UnlockFailed(e) => f.write_fmt(format_args!("unable to release lock with {e}")),
            Error::WouldBlock => f.write_str("acquiring lock would block"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::CreateFailed(err) => Some(err),
            _ => None,
        }
    }
}

/// Lock type for creating results.
pub type Result<T> = result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time};

    #[test]
    fn test_lock() {
        let workers = 4;
        let jobs = 8;
        let pool = threadpool::ThreadPool::new(workers);
        let start = time::SystemTime::now();
        for _ in 0..jobs {
            pool.execute(move || {
                let lock = NamedLock::create("cross-rs-test-lock").unwrap();
                let _guard = lock.lock().unwrap();
                thread::sleep(time::Duration::from_millis(500));
            });
        }
        pool.join();

        // should be ~4s elapsed, due to the locks
        let end = time::SystemTime::now();
        let diff = end.duration_since(start).unwrap();
        assert!((4000..=6000).contains(&diff.as_millis()));
    }
}
