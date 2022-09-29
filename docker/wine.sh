#!/bin/bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    install_packages wget

    dpkg --add-architecture i386

    # add repository for latest wine version and install from source
    # hardcode version, since we might want to avoid a version later.
    wget -nc https://dl.winehq.org/wine-builds/winehq.key

    # workaround for wine server synchronization, see #1035
    # we need to ensure the keys are now stored in `/etc/apt/keyrings`,
    # which were previously stored in `/usr/share/keyrings`, and ensure
    # our sources list searches for the right location.
    mkdir -p /etc/apt/keyrings
    mv winehq.key /etc/apt/keyrings/winehq-archive.key

    wget -nc https://dl.winehq.org/wine-builds/ubuntu/dists/focal/winehq-focal.sources
    mv winehq-focal.sources /etc/apt/sources.list.d/
    sed -i s@/usr/share/keyrings/@/etc/apt/keyrings/@ /etc/apt/sources.list.d/winehq-focal.sources || true

    apt-get update
    apt install --no-install-recommends --assume-yes \
        "winehq-stable=7.0.0.0~focal-1"

    purge_packages
}

main "${@}"
