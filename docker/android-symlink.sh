#!/usr/bin/env bash
# shellcheck disable=SC2125,SC2207

set -x
set -euo pipefail

main() {
    local arch="${1}"
    local target="${2}"
    local libdir="/android-ndk/lib64/clang/"*"/lib/linux/${arch}/"
    local expanded=($(echo "/android-ndk/lib64/clang/"*"/lib/linux/${arch}/"))

    if [[ "${#expanded[@]}" == "1" ]] && [[ "${expanded[0]}" != "${libdir}" ]]; then
        libdir=$(realpath "/android-ndk/lib64/clang/"*"/lib/linux/${arch}/")

        # In Android NDK versions r23-beta3, libgcc has been replaced by libunwind
        # Older Rust versions always link to libgcc, so we need a symlink.
        # https://github.com/rust-lang/rust/pull/85806
        if [[ -f "${libdir}/libunwind.a" ]]; then
            ln -s "${libdir}/libunwind.a" "${libdir}/libgcc.a"
        fi
    fi

    # older SDK versions install the libraries directly in the lib directory.
    local sysroot=/android-ndk/sysroot
    if [[ "${ANDROID_SYSTEM_NONE}" != "1" ]]; then
        if [[ -d "${sysroot}/usr/lib/${target}/" ]]; then
            cp "${sysroot}/usr/lib/${target}/${ANDROID_SDK}/libz.so" /system/lib/
        else
            cp "${sysroot}/usr/lib/libz.so" /system/lib/
        fi
    fi

    # later NDK versions switch to using `llvm-${tool}` rather than `${target}-tool`
    # want to ensure we just have backwards-compatible aliases
    local tool=
    local tool_src=
    local tool_dst=
    for tool in ar as nm objcopy objdump ranlib readelf size string strip; do
        tool_src="/android-ndk/bin/llvm-${tool}"
        tool_dst="/android-ndk/bin/${target}-${tool}"
        if [[ ! -f "${tool_dst}" ]] && [[ -f "${tool_src}" ]]; then
            ln -s "${tool_src}" "${tool_dst}"
        elif [[ "${tool}" == "ld" ]] && [[ ! -f "${tool_dst}" ]]; then
            ln -s "/android-ndk/bin/${tool}" "${tool_dst}"
        fi
    done

    rm "${0}"
}

main "${@}"
