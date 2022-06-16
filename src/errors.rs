pub use color_eyre::Section;
pub use eyre::Context;
pub use eyre::Result;

pub fn install_panic_hook() -> Result<()> {
    color_eyre::config::HookBuilder::new()
        .display_env_section(false)
        .install()
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("`{command}` failed with exit code: {status}")]
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
    /// Attach valuable information to this CommandError
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
