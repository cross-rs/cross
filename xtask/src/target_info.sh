#!/usr/bin/env bash
#
# this script can be invoked as follows:
#       export TARGET=aarch64-unknown-linux-musl
#       docker run -e TARGET ghcr.io/cross-rs/"$TARGET":main bash -c "`cat extract_target_info.sh`"
#
# the output will be similar to the following:
#       | `aarch64-unknown-linux-musl`         | 1.2.0  | 9.2.0   | ✓   | 5.1.0 |
#
# in short, it recreates the table except for the test section in README.md.

set -eo pipefail

if [[ "${VERBOSE}" == 1 ]]; then
    set -x
fi

# shellcheck disable=SC2153
target="${TARGET}"
arch="${target//-*/}"

extract_regex_version() {
    # executing shared libraries outputs to stderr, rest to stdout
    version="$($1 --version 2>&1)"
    if [[ "${version}" =~ $2 ]]; then
        echo "${BASH_REMATCH[1]}"
    else
        echo "Unable to match $3 version info for ${target}." 1>&2
        exit 1
    fi
}

max_glibc_version() {
    # glibc versions have the following format:
    #   `libc-$major-$minor.so.$abi`, where the `.$abi` may be optional.
    # shellcheck disable=SC2207
    local -a paths=( $(ls "${1}"/libc-[0-9]*.[0-9]*.so* 2>/dev/null)  )
    # shellcheck disable=SC2128
    [ -z "$paths" ] && return 0
    local major=0
    local minor=0
    local version
    local x
    local y
    local is_larger

    for i in "${!paths[@]}"; do
        file=$(basename "${paths[$i]}")
        version="${file//libc-/}"
        x=$(echo "${version}" | cut -d '.' -f 1)
        y=$(echo "${version}" | cut -d '.' -f 2)
        is_larger=

        if [ "${x}" -gt "${major}" ]; then
            is_larger=1
        elif [ "${x}" -eq "${major}" ] && [ "${y}" -gt "${minor}" ]; then
            is_larger=1
        fi

        if [ -n "${is_larger}" ]; then
            major="${x}"
            minor="${y}"
        fi
    done

    echo "${major}.${minor}"
}

max_solaris_libc_version() {
    # solaris libc versions have the following format:
    #  67: 0000000000000000     0 OBJECT  GLOBAL DEFAULT  ABS SUNW_1.21.1
    local major=0
    local minor=0
    local patch=0
    local version
    local x
    local y
    local z
    local is_larger

    for version in "${@}"; do
        x=$(echo "${version}" | cut -d '.' -f 1)
        y=$(echo "${version}" | cut -d '.' -f 2)
        z=$(echo "${version}" | cut -d '.' -f 3)
        is_larger=

        if [ "${x}" -gt "${major}" ]; then
            is_larger=1
        elif [ "${x}" -eq "${major}" ] && [ "${y}" -gt "${minor}" ]; then
            is_larger=1
        elif [ -z "${patch}" ]; then
            is_larger=1
        elif [ -n "${z}" ] && [ "${x}" -eq "${major}" ] && [ "${y}" -eq "${minor}" ] && [ "${z}" -gt "${patch}" ]; then
            is_larger=1
        fi

        if [ -n "${is_larger}" ]; then
            major="${x}"
            minor="${y}"
            patch="${z}"
        fi
    done

    result="${major}.${minor}"
    if [ -n "${patch}" ]; then
        result="${result}.${patch}"
    fi
    echo "${result}"
}

readelf_all() {
    # weirdly, readelf -a can produce a non-zero error code.
    set +e
    readelf -a "$1"
    set -e
}

read_solaris_libc() {
    # we can read the libc version from the libc symbols
    # first, we need to use our compiler name to get the libdir
    #    67: 0000000000000000     0 OBJECT  GLOBAL DEFAULT  ABS SUNW_1.21.1
    # there will be many of these, so we want to grab the highest one.
    local libc_so="${1}"
    lines=$(readelf_all "${libc_so}" | grep 'ABS SUNW_')
    lines=$(echo "${lines}" | grep -o 'ABS .*')
    # shellcheck disable=SC2207
    libc_versions=($(echo "$lines" | cut -d ' ' -f 2 | cut -d '_' -f 2))
    max_solaris_libc_version "${libc_versions[@]}"
}


# output variables
libc=
cc=
cxx=
qemu=

# select toolchain information
compiler_suffix="${target//-/_}"
cc_var="CC_${compiler_suffix}"
cxx_var="CXX_${compiler_suffix}"
cc_regex=
case "${target}" in
    *-*-android*)
        cc_regex=".* clang version ([0-9]+.[0-9]+.[0-9]+) .*"
        ;;
    *-*-*-musl*)
        cc_regex=".*gcc \(GCC\) ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *-*-linux-gnu*)
        cc_regex=".*gcc \(.*\) ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *-*-windows-gnu*)
        # MinGW only reports major/minor versions, and can
        # have a -posix or -win32 suffix, eg: 7.5-posix
        cc_regex=".*gcc.* \(GCC\) ([0-9]+.[0-9]+).*"
        ;;
    *-*-freebsd)
        cc_regex=".*gcc \(GCC\) ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *-*-netbsd)
        cc_regex=".*gcc \(.*\) ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *-*-dragonfly)
        cc_regex=".*gcc \(GCC\) ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *-*-solaris)
        cc_regex=".*gcc \(GCC\) ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *-*-illumos)
        cc_regex=".*gcc \(GCC\) ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *-*-emscripten)
        cc_regex="clang version ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *-none-*)
        cc_regex=".*gcc \(.*\) ([0-9]+.[0-9]+.[0-9]+).*"
        ;;
    *)
        echo "TODO: Currently unsupported"  1>&2
        exit 1
        ;;
esac

# select qemu arch
qarch="${arch}"
case "${arch}" in
    arm*)
        qarch="arm"
        ;;
    i*86)
        qarch="i386"
        ;;
    powerpc)
        qarch="ppc"
        ;;
    powerpc64)
        qarch="ppc64"
        ;;
    powerpc64le)
        qarch="ppc64le"
        ;;
    riscv64*)
        qarch="riscv64"
        ;;
esac
qemu_regex="qemu-${qarch} version ([0-9]+.[0-9]+.[0-9]+).*"

# evaluate our toolchain info
cc_bin=
cxx_bin=
case "${target}" in
    i*86-unknown-linux-gnu | x86_64-unknown-linux-gnu)
        cc_bin="gcc"
        cxx_bin="g++"
        ;;
    thumb*-none-eabi* | arm*-none-eabi*)
        # the ARM/THUMB targets don't have a CC_${compiler_suffix}
        cc_bin=arm-none-eabi-gcc
        cxx_bin=arm-none-eabi-g++
        ;;
    *-*-emscripten)
        cc_bin="${EMSDK}/upstream/bin/clang"
        cxx_bin="${cc_bin}"
        ;;
    *)
        cc_bin="${!cc_var}"
        cxx_bin="${!cxx_var}"
        ;;
esac
cc=$(extract_regex_version "${cc_bin}" "${cc_regex}" compiler)
if command -v "${cxx_bin}" &>/dev/null; then
    # test we can compile a c++ program that requires the c++ stdlib
    cat <<EOT >> main.cc
#include <iostream>
int main() {
    std::cout << "Testing this" << std::endl;
}
EOT
    cxx_flags=()
    if [[ "${target}" == *-none-* ]]; then
        cxx_flags=("${cxx_flags[@]}" "-nostartfiles")
    fi
    if [[ "${target}" == *-*-emscripten ]]; then
        # need to switch to the real c++ compiler here, not the wrapper
        cxx_bin=emcc
        # shellcheck disable=SC2206
        cxx_flags=($EMCC_CFLAGS)
    fi
    if "${cxx_bin}" "${cxx_flags[@]}" main.cc >/dev/null 2>&1; then
        cxx=1
    fi
fi

case "${target}" in
    *-*-android*)
        libc="${cc}"
        ;;
    *-*-*-musl*)
        toolchain_prefix="${!cc_var//-gcc/}"
        libdir="/usr/local/${toolchain_prefix}/lib"
        libc_regex=".*Version ([0-9]+.[0-9]+.[0-9]+).*"
        if [[ "${arch}" = i[3-7]86 ]] || [ "${arch}" == x86_64 ]; then
            libc_cmd="${libdir}/libc.so"
        else
            libc_cmd="qemu-${qarch} ${libdir}/libc.so"
            if ! command -v "qemu-${qarch}" &>/dev/null; then
                echo "Unable to get qemu version for ${target}: qemu not found." 1>&2
                exit 1
            fi
        fi
        libc=$(extract_regex_version "${libc_cmd}" "${libc_regex}" libc)
        ;;
    arm-unknown-linux-gnueabihf)
        # this is for crosstool-ng-based images with glibc
        libdir="/x-tools/${target}/${target}/sysroot/lib/"
        libc=$(max_glibc_version "${libdir}")
        ;;
    i*86-unknown-linux-gnu)
        libdir="/lib/x86_64-linux-gnu/"
        libc=$(max_glibc_version "${libdir}")
        ;;
    x86_64-unknown-linux-gnu)
        libdir="/lib64/"
        libc=$(max_glibc_version "${libdir}")
        if [ "$libc" == "" ]; then
            libdir="/lib/x86_64-linux-gnu/"
            libc=$(max_glibc_version "${libdir}")
        fi
        ;;
    riscv64gc-unknown-linux-gnu)
        libc="$(dpkg-query --showformat='${Version}' --show libc6-riscv64-cross | cut -d- -f1)"
        ;;
    *-*-linux-gnu*)
        toolchain_prefix="${!cc_var//-gcc/}"
        libdir="/usr/${toolchain_prefix}/lib"
        libc=$(max_glibc_version "${libdir}")
        ;;
    *-*-windows-gnu)
        # no libc, intentionally omitted.
        ;;
    *-*-freebsd)
        # we write the FreeBSD version to /opt/freebsd-version
        # the symbol versioning can be found here:
        #   https://wiki.freebsd.org/SymbolVersioning
        version=$(cat /opt/freebsd-version)
        if [[ "${version}" =~ ([0-9]+)\.([0-9]+)" ("[A-Za-z]+")" ]]; then
            major_version="${BASH_REMATCH[1]}"
            minor_version="${BASH_REMATCH[2]}"
            case "${major_version}" in
                7)
                    libc="1.0"
                    ;;
                8)
                    libc="1.1"
                    ;;
                9)
                    libc="1.2"
                    ;;
                10)
                    libc="1.3"
                    ;;
                11)
                    libc="1.4"
                    ;;
                12)
                    libc="1.5"
                    ;;
                13)
                    libc="1.6"
                    ;;
                *)
                    echo "Invalid FreeBSD version, got ${major_version}.${minor_version}." 1>&2
                    exit 1
                    ;;
            esac
        else
            echo "Unable to get libc version for ${target}: invalid FreeBSD release found." 1>&2
            exit 1
        fi
        ;;
    *-*-netbsd)
        # We can read the NetBSD version from the libc symbols.
        # The output follows:
        #  NetBSD                0x00000004      IDENT 902000000 (9.2.0)
        libdir="/usr/local/${target}/lib"
        version=$(readelf_all "${libdir}"/libc.so | grep NetBSD | head -n 1)
        if [[ "${version}" =~ .+" ("([0-9]+)"."([0-9]+)"."([0-9]+)")" ]]; then
            major_version="${BASH_REMATCH[1]}"
            minor_version="${BASH_REMATCH[2]}"
            patch_version="${BASH_REMATCH[3]}"
            libc="${major_version}.${minor_version}.${patch_version}"
        else
            echo "Unable to get libc version for ${target}: invalid NetBSD release found." 1>&2
            exit 1
        fi
        ;;
    *-*-dragonfly)
        # we write the Dragonfly version to /opt/dragonfly-version
        version=$(cat /opt/dragonfly-version)
        if [[ "${version}" =~ ([0-9]+)\.([0-9]+)\.([0-9]+)"_REL" ]]; then
            major_version="${BASH_REMATCH[1]}"
            minor_version="${BASH_REMATCH[2]}"
            patch_version="${BASH_REMATCH[3]}"
            libc="${major_version}.${minor_version}.${patch_version}"
        else
            echo "Unable to get libc version for ${target}: invalid Dragonfly release found." 1>&2
            exit 1
        fi
        ;;
    *-*-solaris)
        prefix="${cc_bin//-gcc/}"
        libdir="/usr/local/${prefix}/lib"
        libc=$(read_solaris_libc "${libdir}"/libc.so)
        ;;
    *-*-illumos)
        libdir="/usr/local/${target}/sysroot/lib"
        libc=$(read_solaris_libc "${libdir}"/libc.so)
        ;;
    *-*-emscripten)
        # we want the emsdk version, which is the image version
        libc_cmd=emcc
        libc_regex="emcc \(.* GNU ld) ([0-9]+.[0-9]+.[0-9]+).*"
        libc=$(extract_regex_version "${libc_cmd}" "${libc_regex}" libc)
        ;;
    thumb*-none-eabi* | arm*-none-eabi*)
        # newlib kinda sucks. just query for the install package
        pkg=$(dpkg --get-selections | grep -v deinstall | grep newlib | head -n 1)
        pkg=$(echo "${pkg}" | cut -f 1)
        version=$(dpkg-query --showformat='${Version}' --show "${pkg}")
        if [[ "${version}" =~ ([0-9]+)"."([0-9]+)"."([0-9]+)[^0-9].* ]]; then
            major_version="${BASH_REMATCH[1]}"
            minor_version="${BASH_REMATCH[2]}"
            patch_version="${BASH_REMATCH[3]}"
            libc="${major_version}.${minor_version}.${patch_version}"
        else
            echo "Unable to get libc version for ${target}: invalid THUMB release found." 1>&2
            exit 1
        fi
        ;;
    *)
        echo "TODO: Currently unsupported" 1>&2
        exit 1
        ;;
esac

if command -v "qemu-${qarch}" &>/dev/null; then
    qemu=$(extract_regex_version "qemu-${qarch}" "${qemu_regex}" qemu)
fi

# format our output
printf "| %-36s |" "\`${target}\`"
if [ "$libc" != "" ]; then
    printf " %-6s |" "${libc}"
else
    printf " N/A    |"
fi
if [ "$cc" != "" ]; then
    printf " %-7s |" "${cc}"
else
    printf " N/A     |"
fi
if [ "$cxx" != "" ]; then
    printf " ✓   |"
else
    printf "     |"
fi
if [ "$qemu" != "" ]; then
    printf " %-5s |" "${qemu}"
else
    printf " N/A   |"
fi
if [ "${HAS_TEST}" != "" ]; then
    printf "   ✓    |"
else
    printf "       |"
fi
printf "\n"
