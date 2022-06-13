#!/usr/bin/env bash

echo "Running rustfmt and clippy checks."

set -ex

flags=(--all-features --all-targets --workspace)
cargo fmt -- --check
cargo clippy "${flags[@]}" -- --deny warnings
if cargo +nightly >/dev/null 2>&1; then
    cargo +nightly clippy "${flags[@]}" -- --deny warnings
fi
