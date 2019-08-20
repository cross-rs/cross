#[cfg(not(target_os = "windows"))]
use libc;
#[cfg(not(target_os = "windows"))]
use std::ffi::CStr;

#[cfg(target_os = "windows")]
pub fn group() -> u32 {
    1000
}

#[cfg(not(target_os = "windows"))]
pub fn group() -> u32 {
    unsafe { libc::getgid() }
}

#[cfg(target_os = "windows")]
pub fn user() -> u32 {
    1000
}

#[cfg(not(target_os = "windows"))]
pub fn user() -> u32 {
    unsafe { libc::getuid() }
}

#[cfg(target_os = "windows")]
pub fn username() -> String {
    use std::ptr;

    use winapi::um::winbase::GetUserNameW;

    unsafe {
        let mut size = 0;
        GetUserNameW(ptr::null_mut(), &mut size);

        if size == 0 {
            return "".to_owned()
        }

        let mut username = Vec::with_capacity(size as usize);

        if GetUserNameW(username.as_mut_ptr(), &mut size) == 0 {
            return "".to_owned();
        }

        // Remove trailing space in user name.
        username.set_len((size - 1) as usize);

        String::from_utf16(&username).unwrap()
    }
}

#[cfg(not(target_os = "windows"))]
pub fn username() -> String {
    unsafe {
        CStr::from_ptr((*libc::getpwuid(user())).pw_name)
            .to_string_lossy()
            .into_owned()
    }
}
