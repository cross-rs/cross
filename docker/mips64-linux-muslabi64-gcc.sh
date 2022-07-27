#!/bin/bash

# this fixes an issue of missing symbols from the command lines
# these soft-float routines are required even for hard-float targets.
#   (strtod.lo): undefined reference to symbol '__trunctfsf2@@GCC_3.0'

set -x
set -euo pipefail

main() {
    if [[ $# -gt 0 ]]; then
        exec mips64-linux-musl-gcc "${@}" -lgcc -static-libgcc
    else
        exec mips64-linux-musl-gcc "${@}"
    fi
}

main "${@}"
