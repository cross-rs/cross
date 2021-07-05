#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local uname_arch=$(uname -m)
    local version=3.20.5

    local arch=
    case "${uname_arch}" in
        x86_64)
            arch="x86_64"
            ;;
        arm64)
            arch="aarch64"
            ;;
        *)
            echo "unsupported architecture"
            exit 1
            ;;
    esac

    install_packages curl

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://github.com/Kitware/CMake/releases/download/v${version}/cmake-${version}-Linux-${arch}.sh" -o cmake.sh
    sh cmake.sh --skip-license --prefix=/usr/local

    popd

    purge_packages

    rm -rf "${td}"
    rm -rf /var/lib/apt/lists/*
    rm "${0}"
}

main "${@}"
