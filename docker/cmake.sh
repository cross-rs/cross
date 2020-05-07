#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local version=3.17.2

    local dependencies=(curl)

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --assume-yes --no-install-recommends $dep
            purge_list+=( $dep )
        fi
    done

    local td="$(mktemp -d)"

    pushd $td

    curl -sSfL "https://github.com/Kitware/CMake/releases/download/v${version}/cmake-${version}-Linux-x86_64.sh" -o cmake.sh
    sh cmake.sh --skip-license --prefix=/usr/local

    popd

    if (( ${#purge_list[@]} )); then
      apt-get purge --assume-yes --auto-remove ${purge_list[@]}
    fi

    rm -rf $td
    rm -rf /var/lib/apt/lists/*
    rm $0
}

main "${@}"
