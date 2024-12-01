#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local arch="${1}"

    # python3 is still needed for newer NDK versions, just since it
    # simplifies making symlinks even though the toolchain is prebuilt
    install_packages curl python3
    get_ndk_info
    if [[ "${NDK_VERSION}" -le 9 ]]; then
        install_packages bzip2
    else
        install_packages unzip
    fi

    local td
    td="$(mktemp -d)"

    pushd "${td}"
    curl --retry 3 -sSfL "${NDK_URL}" -O
    if [[ "${NDK_VERSION}" -le 9 ]]; then
        tar -xjf "${NDK_FILENAME}"
    else
        unzip -q "${NDK_FILENAME}"
    fi
    rm "${NDK_FILENAME}"
    pushd "android-ndk-${ANDROID_NDK}"
    # android NDK versions <= 13 error without the verbose flag
    local build_cmd=
    local api=
    if [[ "${NDK_VERSION}" -le 11 ]]; then
        build_cmd=make-standalone-toolchain.sh
        api=--platform="android-${ANDROID_SDK}"
    else
        build_cmd=make_standalone_toolchain.py
        api=--api="${ANDROID_SDK}"
    fi
    "./build/tools/${build_cmd}" \
        --install-dir=/android-ndk \
        --arch="${arch}" \
        "${api}" \
        --verbose

    # the android bash script installs the executables with 750, not 755
    # permissions, and the other files without read permissions.
    if [[ "${NDK_VERSION}" -le 11 ]]; then
        chmod -R 755 /android-ndk/bin
        chmod -R 755 /android-ndk/libexec
        chmod -R +r /android-ndk
    fi

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

get_ndk_info() {
    local ndk_os=linux
    local ndk_platform="${ndk_os}-x86_64"
    # format is generally r21d, r25b, etc. it can however, be r24, for example.
    NDK_VERSION=$(echo "${ANDROID_NDK}" | tr -dc '0-9')
    # android NDK 23 and higher moved from `linux-x86_64` to `linux`
    if [[ "${NDK_VERSION}" -ge 23 ]]; then
        NDK_FILENAME="android-ndk-${ANDROID_NDK}-${ndk_os}.zip"
    elif [[ "${NDK_VERSION}" -le 9 ]]; then
        NDK_FILENAME="android-ndk-${ANDROID_NDK}-${ndk_platform}.tar.bz2"
    else
        NDK_FILENAME="android-ndk-${ANDROID_NDK}-${ndk_platform}.zip"
    fi
    if [[ "${NDK_VERSION}" -le 9 ]]; then
        NDK_URL="https://dl.google.com/android/ndk/${NDK_FILENAME}"
    else
        NDK_URL="https://dl.google.com/android/repository/${NDK_FILENAME}"
    fi
    export NDK_VERSION
    export NDK_FILENAME
    export NDK_URL
}

main "${@}"
