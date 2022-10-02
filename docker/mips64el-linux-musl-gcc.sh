#!/bin/bash

# this linker wrapper works around the missing soft-fp routine __trunctfsf2
# this affects rust versions with compiler-builtins <= 0.1.77,
# which affects toolchains older than 1.65 which require the `-lgcc`
# linker flag to provide the missing builtin.
# https://github.com/rust-lang/compiler-builtins/pull/483

set -x
set -euo pipefail

main() {
    if (( CROSS_RUSTC_MINOR_VERSION >= 65 )) || [[ $# -eq 0 ]]; then
        exec mips64el-linux-musl-gcc "${@}"
    else
        exec mips64el-linux-musl-gcc "${@}" -lgcc -static-libgcc
    fi
}

main "${@}"
