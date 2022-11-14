#!/usr/bin/env bash
# shellcheck disable=SC1091,SC1090

# test to see that foreign toolchains work

set -x
set -eo pipefail

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
. "${ci_dir}"/shared.sh

main() {
    local td=

    retry cargo fetch
    cargo build
    export CROSS="${PROJECT_HOME}/target/debug/cross"

    td="$(mkcargotemp -d)"

    pushd "${td}"
    cargo init --bin --name foreign_toolchain
    # shellcheck disable=SC2016
    echo '# Cross.toml
[build]
default-target = "x86_64-unknown-linux-musl"

[target."x86_64-unknown-linux-musl"]
image.name = "alpine:edge"
image.toolchain = ["x86_64-unknown-linux-musl"]
pre-build = ["apk add --no-cache gcc musl-dev"]' >"${CARGO_TMP_DIR}"/Cross.toml

    "$CROSS" run -v

    local tmp_basename
    tmp_basename=$(basename "${CARGO_TMP_DIR}")
    "${CROSS_ENGINE}" images --format '{{.Repository}}:{{.Tag}}' --filter 'label=org.cross-rs.for-cross-target' | grep "cross-custom-${tmp_basename}" | xargs -t "${CROSS_ENGINE}" rmi

    echo '# Cross.toml
[build]
default-target = "x86_64-unknown-linux-gnu"

[target.x86_64-unknown-linux-gnu]
pre-build = [
    "apt-get update && apt-get install -y libc6 g++-x86-64-linux-gnu libc6-dev-amd64-cross",
]

[target.x86_64-unknown-linux-gnu.env]
passthrough = [
    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc",
    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER=/qemu-runner x86_64",
    "CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc",
    "CXX_x86_64_unknown_linux_gnu=x86_64-linux-gnu-g++",
]

[target.x86_64-unknown-linux-gnu.image]
name = "ubuntu:22.04"
toolchain = ["aarch64-unknown-linux-gnu"]
    ' >"${CARGO_TMP_DIR}"/Cross.toml

    "$CROSS" build -v

    "${CROSS_ENGINE}" images --format '{{.Repository}}:{{.Tag}}' --filter 'label=org.cross-rs.for-cross-target' | grep "cross-custom-${tmp_basename}" | xargs "${CROSS_ENGINE}" rmi

    popd

    rm -rf "${td}"
}

main
