#!/bin/bash

# this linker wrapper works around the missing sync `sync_X_and_fetch`
# routines. this affects rust versions with compiler-builtins <= 0.1.77,
# which has not yet been merged into stable. this requires the `-lgcc`
# linker flag to provide the missing builtin.
# https://github.com/rust-lang/compiler-builtins/pull/484

set -x
set -euo pipefail

main() {
    if [[ $# -eq 0 ]]; then
        exec arm-linux-musleabi-gcc "${@}"
    else
        exec arm-linux-musleabi-gcc "${@}" -lgcc -static-libgcc
    fi
}

main "${@}"
