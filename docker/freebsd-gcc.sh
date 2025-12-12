#!/bin/bash

# the freebsd images need libstdc++ to be linked as well
# otherwise, we get `undefined reference to `std::ios_base::Init::Init()'`

set -euo pipefail

main() {
    if [[ $# -eq 0 ]]; then
        exec "${CROSS_TOOLCHAIN_PREFIX}gcc" "${@}"
    else
        exec "${CROSS_TOOLCHAIN_PREFIX}gcc" "${@}" -lc++ -lstdc++
    fi
}

main "${@}"
