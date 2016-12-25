use libc;

pub fn group() -> u32 {
    unsafe { libc::getgid() }
}

pub fn user() -> u32 {
    unsafe { libc::getuid() }
}
