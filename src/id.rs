use std::ffi::CStr;

use libc;

pub fn group() -> u32 {
    unsafe { libc::getgid() }
}

pub fn user() -> u32 {
    unsafe { libc::getuid() }
}

pub fn username() -> String {
    unsafe {
        CStr::from_ptr((*libc::getpwuid(user())).pw_name)
            .to_string_lossy()
            .into_owned()
    }
}
