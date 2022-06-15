#!/usr/bin/env bash

echo "Running cargo fmt and tests"

set -ex

flags=(--all-features --all-targets --workspace)
cargo fmt -- --check
cargo test "${flags[@]}"
