#![deny(missing_debug_implementations, rust_2018_idioms)]

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    cross::run()?;
    Ok(())
}
