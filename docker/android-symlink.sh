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
    local ndk_libdir="${sysroot}/usr/lib/${target}/"
    if [[ "${ANDROID_SYSTEM_NONE}" != "1" ]]; then
        if [[ -d "${ndk_libdir}/" ]]; then
            cp "${ndk_libdir}/${ANDROID_SDK}/libz.so" /system/lib/
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

    # this is required for CMake builds, since the first pass doesn't
    # add on the SDK API level to the linker search path. for example,
    # it will set the linker search path to `${sysroot}/usr/lib/${target}/`,
    # but not to `${sysroot}/usr/lib/${target}/${ANDROID_SDK}`. this isn't
    # fixable seemingly with **any** environment variable or CMake option:
    # cmake with `CMAKE_ANDROID_STANDALONE_TOOLCHAIN` seemingly ignores:
    #   - `LD_LIBRARY_PATH`
    #   - `CMAKE_CXX_IMPLICIT_LINK_DIRECTORIES`
    #   - `CMAKE_C_COMPILER`
    #   - `CMAKE_CXX_COMPILER`
    #
    # running the cmake configuration a second time works, but this isn't
    # adequate. the resulting config sets `CMAKE_CXX_IMPLICIT_LINK_DIRECTORIES`
    # but this is ignored in our toolchain file. likewise, not testing the
    # compiler via `set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)` fails
    # because during the build it will not add the API level to the linker
    # search path.
    local lib=
    local libname=
    if [[ -d "${ndk_libdir}" ]] && [[ -d "${ndk_libdir}/${ANDROID_SDK}" ]]; then
        for lib in "${ndk_libdir}/${ANDROID_SDK}"/*; do
            libname=$(basename "${lib}")
            if [[ ! -f "${ndk_libdir}/${libname}" ]]; then
                ln -s "${lib}" "${ndk_libdir}/${libname}"
            fi
        done
    fi

    rm "${0}"
}

main "${@}"
