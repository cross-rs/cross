#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local version=3.23.1

    install_packages curl

    local td
    td="$(mktemp -d)"
    pushd "${td}"

    local cmake_arch
    local cmake_sha256

    local narch
    narch="$(dpkg --print-architecture)"

    case "${narch}" in
        amd64)
            cmake_arch="linux-x86_64"
            cmake_sha256="da2a9b18c3bfb136917fa1a579aa5316b01c1d6c111043d03f18877ff05bda30"
            ;;
        arm64)
            cmake_arch="linux-aarch64"
            cmake_sha256="86122bdfd030208aa36705ef421a218ccec52a14368020b2d67043af5e45490b"
            ;;
        *)
            echo "Unsupported architecture: ${narch}" 1>&2
            exit 1
            ;;
     esac

    curl --retry 3 -sSfL "https://github.com/Kitware/CMake/releases/download/v${version}/cmake-${version}-${cmake_arch}.sh" -o cmake.sh
    sha256sum --check <<<"${cmake_sha256}  cmake.sh"
    sh cmake.sh --skip-license --prefix=/usr/local
    cmake --version

    popd

    purge_packages

    rm -rf "${td}"
    rm -rf /var/lib/apt/lists/*
    rm "${0}"
}

main "${@}"
