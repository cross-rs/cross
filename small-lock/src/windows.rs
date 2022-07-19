#![cfg(target_family = "windows")]
#![doc(hidden)]

use std::io;
use std::ptr;

use winapi::shared::minwindef::{DWORD, MAX_PATH};
use winapi::shared::winerror::WAIT_TIMEOUT;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::synchapi::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use winapi::um::winbase::{INFINITE, WAIT_ABANDONED, WAIT_OBJECT_0};
use winapi::um::winnt::{HANDLE, WCHAR};

use crate::{Error, Result};

/// Named (or by path) lock.
#[derive(Debug)]
pub struct InnerLock {
    handle: HANDLE,
    _lpname: Vec<WCHAR>,
}

// the logic for the mutex functionality is based off of this example:
// https://docs.microsoft.com/en-us/windows/win32/sync/using-mutex-objects
impl InnerLock {
    pub fn create(name: impl AsRef<str>) -> Result<InnerLock> {
        match is_valid_namespace(name.as_ref()) {
            // SAFETY: safe since it contains valid characters
            true => unsafe { InnerLock::_create(name.as_ref()) },
            false => Err(Error::InvalidCharacter),
        }
    }

    /// # Safety
    ///
    /// Name must have valid characters.
    unsafe fn _create(name: &str) -> Result<InnerLock> {
        // we want the security descriptor structure to be null,
        // so it cannot be inherited by child processes.
        let lpname = to_wide_string(name);
        let handle = CreateMutexW(ptr::null_mut(), 0, lpname.as_ptr());
        match handle.is_null() {
            true => Err(get_last_error()),
            false => Ok(InnerLock {
                handle,
                _lpname: lpname,
            }),
        }
    }

    pub fn lock(&self) -> Result<InnerLockGuard<'_>> {
        self._lock(INFINITE)
    }

    pub fn try_lock(&self) -> Result<InnerLockGuard<'_>> {
        self._lock(0)
    }

    fn _lock(&self, millis: DWORD) -> Result<InnerLockGuard<'_>> {
        // Safe, since the handle must be valid.
        match unsafe { WaitForSingleObject(self.handle, millis) } {
            WAIT_ABANDONED | WAIT_OBJECT_0 => Ok(InnerLockGuard { lock: self }),
            WAIT_TIMEOUT => Err(Error::WouldBlock),
            code => Err(Error::LockFailed(io::Error::from_raw_os_error(code as i32))),
        }
    }

    pub fn unlock(&self) -> Result<()> {
        self._unlock()
    }

    /// # SAFETY
    ///
    /// Safe, since the handle must be valid.
    fn _unlock(&self) -> Result<()> {
        // Safe, since the handle must be valid.
        match unsafe { ReleaseMutex(self.handle) } {
            0 => Err(get_last_error()),
            _ => Ok(()),
        }
    }
}

impl Drop for InnerLock {
    fn drop(&mut self) {
        // SAFETY: safe, since the handle is valid.
        unsafe { CloseHandle(self.handle) };
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

fn is_valid_namespace(name: &str) -> bool {
    // the name cannot have a backslash, and must be <= MAX_PATH characters.
    // https://docs.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createmutexw
    name.len() <= MAX_PATH && !name.contains('/')
}

/// convert a string to an owned wide string
fn to_wide_string(s: &str) -> Vec<WCHAR> {
    // wchar_t is always 16-bit for UTF-16LE encodings.
    assert_eq!(WCHAR::BITS, 16);
    let mut vec: Vec<WCHAR> = s.encode_utf16().collect();
    vec.push(0);
    vec
}

fn get_last_error() -> Error {
    // SAFETY: safe, since GetLastError is always safe.
    let code = unsafe { GetLastError() };
    Error::CreateFailed(io::Error::from_raw_os_error(code as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_wide_string() {
        assert_eq!(to_wide_string(""), vec![0]);
        assert_eq!(
            to_wide_string("hello world"),
            vec![104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 0]
        );
        assert_eq!(
            to_wide_string("모든 사람은 교육을 받을 권리를"),
            vec![
                47784, 46304, 32, 49324, 46988, 51008, 32, 44368, 50977, 51012, 32, 48155, 51012,
                32, 44428, 47532, 47484, 0
            ]
        );
        assert_eq!(
            to_wide_string("🔥🔥🔥🔥🔥"),
            vec![55357, 56613, 55357, 56613, 55357, 56613, 55357, 56613, 55357, 56613, 0]
        );
    }
}
