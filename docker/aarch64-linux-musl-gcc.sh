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
    local flags=()
    local target="aarch64-unknown-linux-musl"
    minor=$(rustc_minor_version)

    if [[ $# -eq 0 ]]; then
        # no workaround
        true
    elif (( minor >= 48 )) && [[ -n "${CROSS_RUST_SYSROOT:-}" ]]; then
        # find compiler builtins from the sysroot. this also ensures
        # if for whatever reason the linker is invoked outside of
        # cross's build system, we link to libgcc instead.
        local dir="${CROSS_RUST_SYSROOT}/lib/rustlib/${target}/lib"
        local builtins
        builtins=$(ls -1 "${dir}"/libcompiler_builtins*.rlib)
        flags+=("${builtins}" -lc)
    else
        flags+=(-lgcc -static-libgcc -lc)
    fi

    exec "${CROSS_TOOLCHAIN_PREFIX}gcc" "${@}" "${flags[@]}"
}

main "${@}"
