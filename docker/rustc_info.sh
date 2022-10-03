#!/usr/bin/env bash
# FIXME: remove this file further on, especially after
# 0.2.5 has been released and we've had a weeks of
# releases on main so people can update images.
# people may use newer versions of cross with older
# images for backwards compatibility, but we don't
# need to guarantee newer images work with older cross
# versions.
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
