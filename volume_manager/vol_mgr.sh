#!/bin/sh

set -eux

/rustup.sh \
    -y \
    --no-modify-path \
    --default-host x86_64-unknown-linux-gnu \
    --default-toolchain ${RUST_TOOLCHAIN}

~/.cargo/bin/rustup component add rust-src
