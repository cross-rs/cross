use crate::docker;
use crate::temp;

use std::sync::atomic::{AtomicBool, Ordering};

pub use color_eyre::Section;
pub use eyre::Context;
pub use eyre::Result;

pub static mut TERMINATED: AtomicBool = AtomicBool::new(false);

pub fn install_panic_hook() -> Result<()> {
    let is_dev = !crate::commit_info().is_empty() || std::env::var("CROSS_DEBUG").is_ok();
    color_eyre::config::HookBuilder::new()
        .display_env_section(false)
        .display_location_section(is_dev)
        .install()
}

/// # Safety
/// Safe as long as we have single-threaded execution.
unsafe fn termination_handler() {
    // we can't warn the user here, since locks aren't signal-safe.
    // we can delete files, since fdopendir is thread-safe, and
    // `openat`, `unlinkat`, and `lstat` are signal-safe.
    //  https://man7.org/linux/man-pages/man7/signal-safety.7.html
    if !TERMINATED.swap(true, Ordering::SeqCst) && temp::has_tempfiles() {
        temp::clean();
    }

    // tl;dr this is a long explanation to say this is thread-safe.
    // due to the risk for UB with this code, it's important we get
    // this right.
    //
    // this code is wrapped in a sequentially-consistent ordering
    // for strong guarantees about signal safety. since all atomics
    // are guaranteed to be lock free, and must ensure consistent swaps
    // among all threads, this means that we should only ever have one
    // entry into the drop, which makes the external commands safe.
    //
    // to quote the reference on signal safety for linux:
    // > In general, a function is async-signal-safe either because it
    // > is reentrant or because it is atomic with respect to signals
    // > (i.e., its execution can't be interrupted by a signal handler).
    //
    // https://man7.org/linux/man-pages/man7/signal-safety.7.html
    //
    // our operations are atomic, and this is the generally-recommended
    // approach for signal-safe functions that modify global state (note
    // that this is C, but the same rules apply except volatile vars):
    // https://wiki.sei.cmu.edu/confluence/display/c/SIG31-C.+Do+not+access+shared+objects+in+signal+handlers
    //
    // even if the execution of the child process was interrupted by a
    // signal handler, it's an external process that doesn't modify the
    // environment variables of the parent, nor is it likely to lock
    // (except on windows). therefore, for most cases, this code inside
    // the atomic lock guard will still be lock-free, and therefore async
    // signal-safe. in general, the implementations for spawning a child
    // process in rust have the following logic:
    // 1. get a rw-lock for the environment variables, and read them.
    // 2. exec the child process
    //
    // the rw-lock allows any number of readers, which since we're not
    // writing any environment variables, should be async-signal safe:
    // it won't deadlock. it could technically lock if we had something
    // writing environment variables to a child process, and the execution
    // was multi-threaded, but we simply don't do that.
    //
    // for spawning the child process, on unix, the spawn is done via
    // `posix_spawnp`, which is async-signal safe on linux, although
    // it is not guaranteed to be async-signal safe on POSIX in general:
    //      https://bugzilla.kernel.org/show_bug.cgi?id=25292
    //
    // even for POSIX, it's basically thread-safe for our invocations,
    // since we do not modify the environment for our invocations:
    // >  It is also complicated to modify the environment of a multi-
    // > threaded process temporarily, since all threads must agree
    // > when it is safe for the environment to be changed. However,
    // > this cost is only borne by those invocations of posix_spawn()
    // > and posix_spawnp() that use the additional functionality.
    // > Since extensive modifications are not the usual case,
    // > and are particularly unlikely in time-critical code,
    // > keeping much of the environment control out of posix_spawn()
    // > and posix_spawnp() is appropriate design.
    //
    // https://pubs.opengroup.org/onlinepubs/009695399/functions/posix_spawn.html
    //
    // on windows, a non-reentrant static mutex is used, so it is
    // definitely not thread safe, but this should not matter.
    //
    // NOTE: there is one major function that is not async-signal safe here:
    // memory allocation and deallocation, which is not async-signal safe.
    // this should only be run once without deadlocking since any
    // atomics are guaranteed to be lock-free. we cannot easily avoid
    // allocation/deallocation, since we would need static global muts
    // for basically everything. `Command::arg` and `Command::new` will
    // internally allocate, and freeing it will deallocate any arguments
    // it was provided. even if we used a static global `Command`, the
    // `io::Result` requires a `Box` or `io::Error`, which would allocate.
    // the alternative would be to use `posix_spawnp` or `CreateProcess`
    // directly, which are async-signal safe and thread-safe, respectively,
    // however, we'd need to store the engine path and the argument list as
    // a global CString and `Vec<CString>`, respectively. this atomic guard
    // makes this safe regardless.
    docker::CHILD_CONTAINER.terminate();

    // all termination exit codes are 128 + signal code. the exit code is
    // 130 for Ctrl+C or SIGINT (signal code 2) for linux, macos, and windows.
    std::process::exit(130);
}

pub fn install_termination_hook() -> Result<()> {
    // SAFETY: safe since single-threaded execution.
    unsafe {
        signal_hook::low_level::register(signal_hook::consts::SIGINT, || termination_handler())
    }
    .map_err(Into::into)
    .map(|_| ())
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("`{command}` failed with {status}")]
    NonZeroExitCode {
        status: std::process::ExitStatus,
        command: String,
        stderr: Vec<u8>,
        stdout: Vec<u8>,
    },
    #[error("could not execute `{command}`")]
    CouldNotExecute {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
        command: String,
    },
    #[error("`{0:?}` output was not UTF-8")]
    Utf8Error(#[source] std::string::FromUtf8Error, std::process::Output),
}

impl CommandError {
    /// Attach valuable information to this [`CommandError`](Self)
    pub fn to_section_report(self) -> eyre::Report {
        match &self {
            CommandError::NonZeroExitCode { stderr, stdout, .. } => {
                let stderr = String::from_utf8_lossy(stderr).trim().to_owned();
                let stdout = String::from_utf8_lossy(stdout).trim().to_owned();
                eyre::eyre!(self)
                    .section(color_eyre::SectionExt::header(stderr, "Stderr:"))
                    .section(color_eyre::SectionExt::header(stdout, "Stdout:"))
            }
            _ => eyre::eyre!(self),
        }
    }
}
