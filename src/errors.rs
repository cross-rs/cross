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
    #[error("`{1}` failed with exit code: {0}")]
    NonZeroExitCode(std::process::ExitStatus, String),
    #[error("could not execute `{0}`")]
    CouldNotExecute(#[source] Box<dyn std::error::Error + Send + Sync>, String),
    #[error("`{0:?}` output was not UTF-8")]
    Utf8Error(#[source] std::string::FromUtf8Error, std::process::Output),
}
