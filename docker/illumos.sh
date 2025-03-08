#!/usr/bin/env bash
# This script is based off of rust-lang/rust's implementation.
#   https://github.com/rust-lang/rust/blob/47f291ec2d9d6e4820cca517e69b3efddec40c20/src/ci/docker/scripts/illumos-toolchain.sh

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local arch="${1}"
    local binutils=2.38
    local gcc=8.4.0
    local target="${arch}-unknown-illumos"
    local build_target="${arch}-pc-solaris2.10"
    local prefix="/usr/local/${target}"
    local sysroot_dir="${prefix}/sysroot"
    local real_sum

    install_packages ca-certificates \
        curl \
        g++ \
        make \
        texinfo \
        wget \
        xz-utils

    local td
    td="$(mktemp -d)"
    pushd "${td}"

    mkdir "${td}"/{binutils,gcc}{,-build} "${td}/illumos"

    local binutils_sum="e316477a914f567eccc34d5d29785b8b0f5a10208d36bbacedcc39048ecfe024"
    download_binutils "${binutils}" "xz"
    real_sum=$(sha256sum "binutils-${binutils}.tar.xz" | cut -d ' ' -f 1)
    if [[ "${binutils_sum}" != "${real_sum}" ]]; then
        echo "Error: invalid hash for binutils." >&2
        exit 1
    fi
    tar -C "${td}/binutils" --strip-components=1 -xJf "binutils-${binutils}.tar.xz"

    local gcc_sum="e30a6e52d10e1f27ed55104ad233c30bd1e99cfb5ff98ab022dc941edd1b2dd4"
    download_gcc "${gcc}" "xz"
    real_sum=$(sha256sum "gcc-${gcc}.tar.xz" | cut -d ' ' -f 1)
    if [[ "${gcc_sum}" != "${real_sum}" ]]; then
        echo "Error: invalid hash for gcc." >&2
        exit 1
    fi
    tar -C "${td}/gcc" --strip-components=1 -xJf "gcc-${gcc}.tar.xz"

    pushd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    popd

    local mach
    case "${arch}" in
        x86_64)
            mach='i386'
            ;;
        *)
            echo "ERROR: unknown architecture: ${arch}" >&2
            exit 1
            ;;
    esac

    local sysroot_version="20181213-de6af22ae73b-v1"
    local sysroot_file="illumos-sysroot-${mach}-${sysroot_version}.tar.gz"
    local sysroot_repo="https://github.com/illumos/sysroot"
    local sysroot_sum="ee792d956dfa6967453cebe9286a149143290d296a8ce4b8a91d36bea89f8112"
    curl --retry 3 -sSfL "${sysroot_repo}/releases/download/${sysroot_version}/${sysroot_file}" -O
    real_sum=$(sha256sum "${sysroot_file}" | cut -d ' ' -f 1)
    if [[ "${sysroot_sum}" != "${real_sum}" ]]; then
        echo "Error: invalid hash for illumos sysroot." >&2
        exit 1
    fi
    mkdir -p "${sysroot_dir}"
    pushd "${sysroot_dir}"
    tar -xzf "${td}/${sysroot_file}"
    popd

    mkdir -p "${prefix}"
    pushd binutils-build
    ../binutils/configure \
        --target="${build_target}" \
        --prefix="${prefix}" \
        --program-prefix="${target}-" \
        --with-sysroot="${sysroot_dir}"
    make "-j$(nproc)"
    make install
    popd

    # note: solaris2.10 is obsolete, so we can't upgrade to GCC 10 till then.
    # for gcc 9.4.0, need `--enable-obsolete`.
    export CFLAGS='-fPIC'
    export CXXFLAGS='-fPIC'
    export CXXFLAGS_FOR_TARGET='-fPIC'
    export CFLAGS_FOR_TARGET='-fPIC'
    mkdir -p "${prefix}"
    pushd gcc-build
    ../gcc/configure \
        --prefix="${prefix}" \
        --target="${build_target}" \
        --program-prefix="${target}-" \
        --with-sysroot="${sysroot_dir}" \
        --enable-languages=c,c++,fortran \
        --disable-libada \
        --disable-libcilkrts \
        --disable-libgomp \
        --disable-libquadmath \
        --disable-libquadmath-support \
        --disable-libsanitizer \
        --disable-libssp \
        --disable-libvtv \
        --disable-lto \
        --disable-multilib \
        --disable-shared \
        --disable-nls \
        --enable-tls \
        --with-gnu-as \
        --with-gnu-ld
    make "-j$(nproc)"
    make install
    popd

    # clean up
    popd

    purge_packages

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
