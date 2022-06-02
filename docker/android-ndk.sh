#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

NDK_URL=https://dl.google.com/android/repository/android-ndk-r21d-linux-x86_64.zip

main() {
    local arch="${1}" \
          api="${2}"

    install_packages curl unzip python

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

    purge_packages

    popd
    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
