#!/bin/bash

# this linker wrapper works around issue https://github.com/rust-lang/rust/issues/46651
# which affects toolchains older than 1.48
# older toolchains require the `-lgcc` linker flag otherwise they fail to link

set -x
set -euo pipefail

main() {
    if (( CROSS_RUSTC_MINOR_VERSION >= 48 )) || [[ $# -eq 0 ]]; then
        # no workaround
        exec aarch64-linux-musl-gcc "${@}"
    else
        # apply workaround
        exec aarch64-linux-musl-gcc "${@}" -lgcc -static-libgcc
    fi
}

main "${@}"
