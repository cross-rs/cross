#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local narch
    local -a packages

    narch="$(dpkg --print-architecture)"
    packages+=("libc6-dev-${TARGET_ARCH}-cross:${narch}")

    # Install crossbuild-essential if CROSSBUILD_ESSENTIAL is set
    if [ -n "${CROSSBUILD_ESSENTIAL:-}" ]; then
        packages+=("crossbuild-essential-${TARGET_ARCH}:${narch}")
    fi

    if ! command -v "${CROSS_TOOLCHAIN_PREFIX}g++" &>/dev/null; then
      packages+=("g++-${TARGET_TRIPLE}:${narch}")
    fi

    if ! command -v "${CROSS_TOOLCHAIN_PREFIX}gfortran" &>/dev/null; then
      packages+=("gfortran-${TARGET_TRIPLE}:${narch}")
    fi

    install_packages "${packages[@]}"

    rm "${0}"
}

main "${@}"
