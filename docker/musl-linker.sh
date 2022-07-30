#!/bin/bash

# this fixes a series of issues affecting musl targets, with missing
# instrinsics that are symbols present in libgcc but not in the compiler
# builtins. this is therefore a temporary workaround until these
# issues are fixed. example issues previously discovered are noted below:

# mips64-unknown-linux-musl
# this linker wrapper works around issue https://github.com/rust-lang/rust/issues/46651
# which affects toolchains older than 1.48
# older toolchains require the `-lgcc` linker flag otherwise they fail to link

# mips64-unknown-linux-gnuabi64
# this fixes an issue of missing symbols from the command lines
# these soft-float routines are required even for hard-float targets.
#   (strtod.lo): undefined reference to symbol '__trunctfsf2@@GCC_3.0'

set -eo

main() {
    if [[ -n "${CROSS_NO_LIBGCC_ROUTINES}" ]] || [[ $# -eq 0 ]]; then
        exec "${CROSS_TARGET_LINKER}" "${@}"
    else
        exec "${CROSS_TARGET_LINKER}" "${@}" -lgcc -static-libgcc
    fi
}

main "${@}"
