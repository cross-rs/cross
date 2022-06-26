#![deny(missing_debug_implementations, rust_2018_idioms)]

pub fn main() -> cross::Result<()> {
    if std::env::var(cross::IN_CROSS_CONTEXT_ENV).is_ok() {
        // HACK: This needs a exec_replace
        let args = std::env::args_os();
        std::process::exit(
            std::process::Command::new("cargo")
                .args(args.skip(1))
                .status()?
                .code()
                .ok_or_else(|| eyre::Report::msg("Cargo process terminated by signal"))?,
        );
    }
    cross::install_panic_hook()?;
    cross::install_termination_hook()?;

    let status = cross::run()?;
    let code = status
        .code()
        .ok_or_else(|| eyre::Report::msg("Cargo process terminated by signal"))?;
    std::process::exit(code)
}
