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
    mv winehq.key /usr/share/keyrings/winehq-archive.key
    wget -nc https://dl.winehq.org/wine-builds/ubuntu/dists/bionic/winehq-bionic.sources
    mv winehq-bionic.sources /etc/apt/sources.list.d/
    apt-get update
    apt install --no-install-recommends --assume-yes \
        "winehq-stable=7.0.0.0~bionic-1"

    purge_packages
}

main "${@}"
