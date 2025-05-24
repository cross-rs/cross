use crate::rustup;

#[test]
fn remove_toolchain_suffixes() {
    // no overrides (default is active)
    assert_eq!(
        rustup::remove_toolchain_suffixes(
            "stable-aarch64-apple-darwin (active, default)\n\
             stable-x86_64-unknown-linux-gnu"
        ),
        vec![
            "stable-aarch64-apple-darwin",
            "stable-x86_64-unknown-linux-gnu",
        ]
    );
    // with overrides (default is not active)
    assert_eq!(
        rustup::remove_toolchain_suffixes(
            "stable-aarch64-apple-darwin (default)\n\
             stable-x86_64-unknown-linux-gnu\n\
             nightly-aarch64-apple-darwin (active)"
        ),
        vec![
            "stable-aarch64-apple-darwin",
            "stable-x86_64-unknown-linux-gnu",
            "nightly-aarch64-apple-darwin",
        ]
    );
}
