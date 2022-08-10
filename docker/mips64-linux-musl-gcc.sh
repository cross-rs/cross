#!/bin/bash

# this linker wrapper works around the missing soft-fp routine __trunctfsf2
# this affects rust versions with compiler-builtins <= 0.1.77,
# which has not yet been merged into stable. this requires the `-lgcc`
# linker flag to provide the missing builtin.
# https://github.com/rust-lang/compiler-builtins/pull/483

set -x
set -euo pipefail

main() {
    if [[ $# -eq 0 ]]; then
        exec mips64-linux-musl-gcc "${@}"
    else
        exec mips64-linux-musl-gcc "${@}" -lgcc -static-libgcc
    fi
}

main "${@}"
