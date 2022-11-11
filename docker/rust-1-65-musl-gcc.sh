#!/bin/bash

# this linker wrapper works around various missing builtins affecting
# rust versopns with compiler-builtins <= 0.1.77, or older than 1.65.
# these require the `-lgcc` linker flag to provide the missing builtin.
# target-specific details include:
#
# armv5te-unknown-linux-musleabi
#   missing sync `sync_X_and_fetch`
#   https://github.com/rust-lang/compiler-builtins/pull/484
#
# mips64-unknown-linux-muslabi64, mips64el-unknown-linux-muslabi64
#   missing soft-fp routine `__trunctfsf2`
#   https://github.com/rust-lang/compiler-builtins/pull/483

set -x
set -euo pipefail

# shellcheck disable=SC1091
. /rustc_info.sh

main() {
    local minor
    minor=$(rustc_minor_version)

    if (( minor >= 65 )) || [[ $# -eq 0 ]]; then
        exec "${CROSS_TOOLCHAIN_PREFIX}gcc" "${@}"
    else
        exec "${CROSS_TOOLCHAIN_PREFIX}gcc" "${@}" -lgcc -static-libgcc
    fi
}

main "${@}"
