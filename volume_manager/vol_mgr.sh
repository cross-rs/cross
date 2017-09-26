#!/bin/sh

set -eux

./rustup.sh \
    -y \
    --no-modify-path \
    --default-host x86_64-unknown-linux-gnu \
    --default-toolchain ${RUST_TOOLCHAIN}

~/.cargo/bin/rustup component add rust-src

# TODO: configurable?
export tag="v0.3.5"
export target="x86_64-unknown-linux-gnu"

. ~/.cargo/env

# Check if Xargo already exists and is usable, otherwise install
/xargo/xargo --version || \
    rm -f /xargo/xargo && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag $tag --target $target --to /xargo

mv ~/.cargo/* /cargo && \
mv ~/.rustup/toolchains/${RUST_TOOLCHAIN}-x86_64-unknown-linux-gnu/* /rust
