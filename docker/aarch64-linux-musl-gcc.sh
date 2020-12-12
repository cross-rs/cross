#!/bin/bash

# this linker wrapper works around issue https://github.com/rust-lang/rust/issues/46651
# which affects toolchains older than 1.48
# older toolchains require the `-lgcc` linker flag otherwise they fail to link

set -euo pipefail

main() {
    local release=
    release=$(rustc -Vv | grep '^release:' | cut -d ':' -f2)
    # NOTE we assume `major` is always "1"
    local minor=
    minor=$(echo "$release" | cut -d '.' -f2)

    if (( minor >= 48 )); then
        # no workaround
        aarch64-linux-musl-gcc "${@}"
    else
        # apply workaround
        aarch64-linux-musl-gcc "${@}" -lgcc
    fi
}

main "${@}"
