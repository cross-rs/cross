#!/usr/bin/env bash
# shellcheck disable=SC1091,SC1090

# test that we correctly handle args after `--`

set -x
set -eo pipefail

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
. "${ci_dir}"/shared.sh

main() {
    local td=
    local parent=
    local target=x86_64-unknown-dragonfly

    rustup toolchain add nightly

    retry cargo fetch
    cargo build
    export CROSS="${PROJECT_HOME}/target/debug/cross"

    td="$(mkcargotemp -d)"
    parent=$(dirname "${td}")
    pushd "${td}"
    cargo init --bin --name "hello" .

    echo '[target.'"${target}"']
build-std = true' > "${parent}/Cross.toml"

    export CROSS_CONTAINER_ENGINE="${CROSS_ENGINE}"
    "${CROSS}" +nightly build --target "${target}" --verbose --

    popd
    rm -rf "${td}"
}

main
