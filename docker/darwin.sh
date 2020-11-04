#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local dependencies=(
        clang
        curl
        gcc
        g++
        make
        patch
        libmpc-dev
        libmpfr-dev
        libgmp-dev
        libssl-dev
        libxml2-dev
        xz-utils
        zlib1g-dev
    )

    apt-get update

    # this must be installed first, otherwise an interactive prompt is
    # triggered by another package install
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends tzdata

    for dep in "${dependencies[@]}"; do
        if ! dpkg -L "${dep}"; then
            apt-get install --assume-yes --no-install-recommends "${dep}"
        fi
    done

    cd /opt
    git clone https://github.com/tpoechtrager/osxcross
    cd osxcross
    curl -L https://s3.dockerproject.org/darwin/v2/MacOSX10.10.sdk.tar.xz -o tarballs/MacOSX10.10.sdk.tar.xz
    UNATTENDED=yes OSX_VERSION_MIN=10.7 ./build.sh
    ln -s /opt/osxcross/target/bin/* /usr/local/bin/

    local purge_list=(
        curl
        gcc
        g++
        make
        patch
        xz-utils
    )

    if (( ${#purge_list[@]} )); then
      apt-get purge --assume-yes --auto-remove "${purge_list[@]}"
    fi
}

main "${@}"
