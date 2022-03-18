#[cfg(not(target_os = "windows"))]
use nix::{
    errno::{errno, Errno},
    unistd::{Gid, Uid},
    Error,
};
#[cfg(not(target_os = "windows"))]
use std::ffi::CStr;

#[cfg(target_os = "windows")]
pub fn group() -> u32 {
    1000
}

#[cfg(not(target_os = "windows"))]
pub fn group() -> u32 {
    Gid::current().as_raw()
}

#[cfg(target_os = "windows")]
pub fn user() -> u32 {
    1000
}

#[cfg(not(target_os = "windows"))]
pub fn user() -> u32 {
    Uid::current().as_raw()
}

#[cfg(target_os = "windows")]
pub fn username() -> Result<Option<String>, String> {
    use std::ptr;

    use winapi::um::winbase::GetUserNameW;

    unsafe {
        let mut size = 0;
        GetUserNameW(ptr::null_mut(), &mut size);

        if size == 0 {
            return Ok(None);
        }

        let mut username = Vec::with_capacity(size as usize);

        if GetUserNameW(username.as_mut_ptr(), &mut size) == 0 {
            return Err("Could not get UserName.".to_owned());
        }

        // Remove null terminator.
        username.set_len((size - 1) as usize);

        Ok(Some(String::from_utf16_lossy(&username)))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn username() -> Result<Option<String>, Error> {
    let name = unsafe {
        Errno::clear();

        let passwd = libc::getpwuid(Uid::current().as_raw());

        if passwd.is_null() {
            let errno = errno();

            if errno == 0 {
                return Ok(None);
            }

            return Err(Errno::from_i32(errno));
        }

        CStr::from_ptr((*passwd).pw_name)
    };

    Ok(Some(name.to_string_lossy().into_owned()))
}
