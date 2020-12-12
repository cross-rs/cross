#!/usr/bin/env bash

set -x
set -euo pipefail

NDK_URL=https://dl.google.com/android/repository/android-ndk-r13b-linux-x86_64.zip

main() {
    local arch="${1}" \
          api="${2}"

    local dependencies=(
        curl
        unzip
        python
    )

    apt-get update
    local purge_list=()
    for dep in "${dependencies[@]}"; do
        if ! dpkg -L "${dep}"; then
            apt-get install --assume-yes --no-install-recommends "${dep}"
            purge_list+=( "${dep}" )
        fi
    done

    local td
    td="$(mktemp -d)"

    pushd "${td}"
    curl --retry 3 -sSfL "${NDK_URL}" -O
    unzip -q android-ndk-*.zip
    rm android-ndk-*.zip
    pushd android-ndk-*
    ./build/tools/make_standalone_toolchain.py \
      --install-dir /android-ndk \
      --arch "${arch}" \
      --api "${api}"

    if (( ${#purge_list[@]} )); then
      apt-get purge --assume-yes --auto-remove "${purge_list[@]}"
    fi

    popd
    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
