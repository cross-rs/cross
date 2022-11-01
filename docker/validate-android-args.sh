#!/usr/bin/env bash
# Ensure the NDK, SDK, and Android versions match to exit
# before a build or even worse, a runner later fails.

set -x
set -euo pipefail

main() {
    local arch="${1}"

    validate_ndk "${arch}"
    validate_sdk
    validate_system
    validate_ndk_sdk "${arch}"
    validate_sdk_system
}

validate_ndk() {
    local arch="${1}"
    local ndk_version=
    ndk_version=$(echo "${ANDROID_NDK}" | tr -dc '0-9')

    case "${arch}" in
        mips|mips64)
            if [[ "${ndk_version}" -ge 17 ]]; then
                echo "Android NDKs r17+ removed support for MIPS architectures." 1>&2
                exit 1
            fi
            ;;
        *)
            ;;
    esac
}

validate_sdk() {
    local invalid_sdk_versions=(6 7 10 11 20 25)
    # shellcheck disable=SC2076
    if [[ "${invalid_sdk_versions[*]}" =~ "${ANDROID_SDK}" ]]; then
        echo "The Android SDK version ${ANDROID_SDK} is not provided by Android and therefore not supported." 1>&2
        exit 1
    fi
}

validate_system() {
    local major_version
    major_version=$(echo "${ANDROID_VERSION}" | cut -d '.' -f 1)
    if [[ "${major_version}" -lt 5 ]]; then
        echo "Invalid Android version ${ANDROID_VERSION}, must be Android 5+." 1>&2
        exit 1
    fi
}

validate_ndk_sdk() {
    local arch="${1}"
    local ndk_version=
    ndk_version=$(echo "${ANDROID_NDK}" | tr -dc '0-9')

    # no minimum version for most 32-bit architectures
    case "${arch}" in
        arm|x86)
            ;;
        mips)
            check_min_sdk_arch "${arch}" 9
            ;;
        arm64|mips64|x86_64)
            check_min_sdk_arch "${arch}" 21
            ;;
        *)
            echo "Unsupported architecture, got ${arch}." 1>&2
            exit 1
            ;;
    esac

    case "${ndk_version}" in
        9)
            check_sdk_range 3 19
            ;;
        10)
            check_sdk_range 3 21
            ;;
        11)
            check_sdk_range 3 24
            ;;
        12|13|14)
            check_sdk_range 9 24
            ;;
        15)
            check_sdk_range 14 26
            ;;
        16)
            check_sdk_range 14 27
            ;;
        17)
            check_sdk_range 14 28
            ;;
        18)
            check_sdk_range 16 28
            ;;
        19)
            check_sdk_range 16 28
            ;;
        20)
            check_sdk_range 16 29
            ;;
        21|22)
            check_sdk_range 21 30
            ;;
        23)
            check_sdk_range 21 31
            ;;
        24)
            check_sdk_range 21 32
            ;;
        25)
            check_sdk_range 21 33
            ;;
        *)
            echo "Currently unsupported NDK version of ${ndk_version}." 1>&2
            echo "If you would like support, please file an issue." 1>&2
            exit 1
            ;;
    esac
}

check_min_sdk_arch() {
    local arch="${1}"
    local minimum="${2}"
    if [[ "${ANDROID_SDK}" -lt "${minimum}" ]]; then
        echo "Invalid SDK version ${ANDROID_SDK} for architecture ${arch}" 1>&2
        echo "The minimum supported SDK version is ${minimum}." 1>&2
        exit 1
    fi
}

check_sdk_range() {
    local lower="${1}"
    local upper="${2}"
    if [[ "${ANDROID_SDK}" -lt "${lower}" ]] || [[ "${ANDROID_SDK}" -gt "${upper}" ]]; then
        echo "Invalid SDK version ${ANDROID_SDK} for NDK version ${ANDROID_NDK}" 1>&2
        echo "Valid SDK versions are ${lower}-${upper}." 1>&2
        exit 1
    fi
}

validate_sdk_system() {
    local major_version
    local minor_version
    major_version=$(echo "${ANDROID_VERSION}" | cut -d '.' -f 1)
    minor_version=$(echo "${ANDROID_VERSION}" | cut -d '.' -f 2)
    local system_version="${major_version}.${minor_version}"
    case "${system_version}" in
        5.0)
            check_sdk_system_equal 21
            ;;
        5.1)
            check_sdk_system_equal 22
            ;;
        6.0)
            check_sdk_system_equal 23
            ;;
        7.0)
            check_sdk_system_equal 24
            ;;
        7.1)
            check_sdk_system_equal 25
            ;;
        8.0)
            check_sdk_system_equal 26
            ;;
        8.1)
            check_sdk_system_equal 27
            ;;
        9.0)
            check_sdk_system_equal 28
            ;;
        10.0)
            check_sdk_system_equal 29
            ;;
        11.0)
            check_sdk_system_equal 30
            ;;
        12.0)
            check_sdk_system_equal 31
            ;;
        12.1)
            # NOTE: also knows as 12L
            check_sdk_system_equal 32
            ;;
        13.0)
            check_sdk_system_equal 33
            ;;
        *)
            echo "Currently unsupported Android system version of ${system_version}." 1>&2
            echo "If you would like support, please file an issue." 1>&2
            exit 1
            ;;
    esac
}

check_sdk_system_equal() {
    local expected=("$@")
    local valid=0

    for version in "${expected[@]}"; do
        if [[ "${ANDROID_SDK}" == "${version}" ]]; then
            valid=1
        fi
    done

    if [[ "${valid}" -ne 1 ]]; then
        # shellcheck disable=SC2145
        echo "Invalid SDK version, got ${ANDROID_SDK} and expected ${expected[@]}." 1>&2
        exit 1
    fi
}

main "${@}"
