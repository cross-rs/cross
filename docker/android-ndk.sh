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

    # clean up unused toolchains to reduce image size
    local triple
    local triples
    local triple_arch="${arch}"
    case "${arch}" in
      arm64)
        triple_arch="aarch64"
        ;;
      x86)
        triple_arch="i686"
        ;;
    esac
    triples=(
      "aarch64-linux-android"
      "arm-linux-androideabi"
      "i686-linux-android"
      "x86_64-linux-android"
    )
    for triple in "${triples[@]}"; do
      if ! [[ "${triple}" =~ ^"${triple_arch}".* ]]; then
        rm -rf "/android-ndk/sysroot/usr/lib/${triple}"
      fi
    done

    purge_packages

    popd
    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
