#!/bin/bash

# this linker wrapper works around issue https://github.com/rust-lang/rust/issues/46651
# which affects toolchains older than 1.48
# older toolchains require the `-lgcc` linker flag otherwise they fail to link

set -x
set -euo pipefail

# shellcheck disable=SC1091
. /rustc_info.sh

main() {
    local minor
    minor=$(rustc_minor_version)

    if (( minor >= 48 )) || [[ $# -eq 0 ]]; then
        # no workaround
        exec "${CROSS_TOOLCHAIN_PREFIX}gcc" "${@}"
    else
        # apply workaround
        exec "${CROSS_TOOLCHAIN_PREFIX}gcc" "${@}" -lgcc -static-libgcc
    fi
}

main "${@}"
