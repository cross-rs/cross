pub use color_eyre::Section;
pub use eyre::Context;
pub use eyre::Result;

pub fn install_panic_hook() -> Result<()> {
    color_eyre::config::HookBuilder::new()
        .display_env_section(false)
        .install()
}
