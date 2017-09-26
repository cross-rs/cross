#!/bin/sh

set -eux

./rustup.sh \
    -y \
    --no-modify-path \
    --default-host x86_64-unknown-linux-gnu \
    --default-toolchain ${RUST_TOOLCHAIN}

~/.cargo/bin/rustup component add rust-src

mv ~/.cargo/* /cargo && \
mv ~/.rustup/toolchains/${RUST_TOOLCHAIN}-x86_64-unknown-linux-gnu/* /rust
cp ~/.rustup/settings.toml /rust/settings.toml
