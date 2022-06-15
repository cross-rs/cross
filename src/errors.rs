use crate::temp;

use std::sync::atomic::{AtomicBool, Ordering};

pub use color_eyre::Section;
pub use eyre::Context;
pub use eyre::Result;

pub static mut TERMINATED: AtomicBool = AtomicBool::new(false);

pub fn install_panic_hook() -> Result<()> {
    color_eyre::config::HookBuilder::new()
        .display_env_section(false)
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

    // EOWNERDEAD, seems to be the same on linux, macos, and bash on windows.
    std::process::exit(130);
}

pub fn install_termination_hook() -> Result<()> {
    // SAFETY: safe since single-threaded execution.
    ctrlc::set_handler(|| unsafe { termination_handler() }).map_err(Into::into)
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
                let stderr = String::from_utf8_lossy(stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(stdout).trim().to_string();
                eyre::eyre!(self)
                    .section(color_eyre::SectionExt::header(stderr, "Stderr:"))
                    .section(color_eyre::SectionExt::header(stdout, "Stdout:"))
            }
            _ => eyre::eyre!(self),
        }
    }
}
