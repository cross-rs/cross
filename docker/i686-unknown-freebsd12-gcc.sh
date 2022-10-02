#!/bin/bash

# the freebsd images need libstdc++ to be linked as well
# otherwise, we get `undefined reference to `std::ios_base::Init::Init()'`

set -x
set -euo pipefail

main() {
    if [[ $# -eq 0 ]]; then
        exec i686-unknown-freebsd12-gcc "${@}"
    else
        exec i686-unknown-freebsd12-gcc "${@}" -lc++ -lstdc++
    fi
}

main "${@}"
