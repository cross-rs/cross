#!/bin/bash

# this works around missing libc routines when compiling
# against a static libstdc++, which we always do on musl after
# https://github.com/cross-rs/cross/issues/902. the reason
# otherwise, we are missing crucial routines as as `setlocale`,
# `__cxa_atexit`, and others.

set -x
set -euo pipefail

main() {
    if [[ $# -eq 0 ]]; then
        exec "${CROSS_TOOLCHAIN_PREFIX}"gcc "${@}"
    else
        exec "${CROSS_TOOLCHAIN_PREFIX}"gcc "${@}" -lc
    fi
}

main "${@}"
