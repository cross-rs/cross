#!/usr/bin/env bash
# Adapted from rust-land/rust x86_64-unknown-linux-gnu-emulated's Dockerfile.
# Commit 2d1e0750792

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local filename="sde-external-8.35.0-2019-03-11-lin.tar.bz2"
    local url="https://github.com/gnzlbg/intel_sde/raw/master/${filename}"

    install_packages \
        bzip2 \
        curl \
        tar

    local td
    td="$(mktemp -d)"

    pushd "${td}"
    curl --retry 3 -sSfL "${url}" -O
    mkdir -p "/opt/intel"
    tar -C "/opt/intel" --strip-components=1 -xjf "${filename}"

    purge_packages

    popd

    rm -rf "${td}"
    rm -rf "${0}"
}

main "${@}"
