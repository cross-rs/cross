#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local version=3.16.2

    local dependencies=(
        curl
        g++
        libssl-dev
        make
        zlib1g-dev
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    local td="$(mktemp -d)"

    pushd $td

    curl https://cmake.org/files/v${version%.*}/cmake-$version.tar.gz | \
        tar --strip-components 1 -xz

    ./bootstrap
    make -j$(nproc)
    make install

    # clean up
    popd

    apt-get purge --auto-remove -y ${purge_list[@]}

    rm -rf $td
    rm $0
}

main "${@}"
