#!/bin/bash

# this linker works around missing builtins in older rust versions.
# we also have custom linker scripts for our static libstdc++ for all versions
# which is found in `musl-symlink.sh`.
#
# for other targets, issues in older versions of compiler-builtins require
# manually linking to libgcc to compensate for missing builtins.
# target-specific details include:
#
# aarch64-unknown-linux-musl (fixed in 1.48)
#   https://github.com/rust-lang/compiler-builtins/pull/377
#
# armv5te-unknown-linux-musleabi (fixed in 1.65)
#   missing sync `sync_X_and_fetch`
#   https://github.com/rust-lang/compiler-builtins/pull/484
#
# mips64-unknown-linux-muslabi64, mips64el-unknown-linux-muslabi64  (fixed in 1.65)
#   missing soft-fp routine `__trunctfsf2`
#   https://github.com/rust-lang/compiler-builtins/pull/483

set -x
set -euo pipefail

main() {
    local minor
    local patched_minor="${CROSS_BUILTINS_PATCHED_MINOR_VERSION:-0}"
    minor=$(rustc_minor_version)

    if [[ $# -eq 0 ]] || [[ "${minor}" -ge "${patched_minor}" ]]; then
        exec "${CROSS_TOOLCHAIN_PREFIX}"gcc "${@}"
    else
        exec "${CROSS_TOOLCHAIN_PREFIX}"gcc "${@}" -lgcc -static-libgcc
    fi
}

# FIXME: the rest of the contents of this file can be removed later on,
# especially after 0.3.0 has been released so we can ensure everyone is
# using a cross version at least as recent as images requiring the rust
# versions provided as environment variables. these functions are wrappers
# around these environment variables for backwards compatibility.
# https://github.com/cross-rs/cross/issues/1046

# NOTE: this will fail if rustc does not provide version
# info, which may happen with a custom toolchain.
rustc_version() {
    rustc -Vv | grep '^release:' | cut -d ':' -f2
}

rustc_major_version() {
    if [[ -z "${CROSS_RUSTC_MAJOR_VERSION:-}" ]]; then
        CROSS_RUSTC_MAJOR_VERSION=$(rustc_version | cut -d '.' -f1)
        export CROSS_RUSTC_MAJOR_VERSION
    fi
    echo "${CROSS_RUSTC_MAJOR_VERSION}"
}

rustc_minor_version() {
    if [[ -z "${CROSS_RUSTC_MINOR_VERSION:-}" ]]; then
        CROSS_RUSTC_MINOR_VERSION=$(rustc_version | cut -d '.' -f2)
        export CROSS_RUSTC_MINOR_VERSION
    fi
    echo "${CROSS_RUSTC_MINOR_VERSION}"
}

rustc_patch_version() {
    if [[ -z "${CROSS_RUSTC_PATCH_VERSION:-}" ]]; then
        CROSS_RUSTC_PATCH_VERSION=$(rustc_version | cut -d '.' -f3)
        export CROSS_RUSTC_PATCH_VERSION
    fi
    echo "${CROSS_RUSTC_PATCH_VERSION}"
}

main "${@}"
