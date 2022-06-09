#![deny(missing_debug_implementations, rust_2018_idioms)]

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let status = cross::run()?;
    let code = status
        .code()
        .ok_or_else(|| eyre::Report::msg("Cargo process terminated by signal"))?;
    std::process::exit(code)
}
