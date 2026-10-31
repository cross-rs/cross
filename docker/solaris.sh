#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local arch="${1}"
    local manufacturer="${2}"

    local binutils=2.38 \
        gcc=8.4.0 \
        target="${arch}-${manufacturer}-solaris2.10"

    install_packages bzip2 \
        ca-certificates \
        curl \
        g++ \
        make \
        patch \
        texinfo \
        wget \
        xz-utils

    local td
    td="$(mktemp -d)"
    pushd "${td}"

    mkdir "${td}"/{binutils,gcc}{,-build} "${td}/solaris"

    download_binutils "${binutils}" "xz"
    tar -C "${td}/binutils" --strip-components=1 -xJf "binutils-${binutils}.tar.xz"

    download_gcc "${gcc}" "xz"
    tar -C "${td}/gcc" --strip-components=1 -xJf "gcc-${gcc}.tar.xz"

    cd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    cd ..

    local mach=
    local lib_arch=
    local sysroot_sum=
    case "${arch}" in
        x86_64)
            mach=i386
            lib_arch=amd64
            sysroot_sum=e82b78c14464cc2dc71f3cdab312df3dd63441d7c23eeeaf34d41d8b947688d3
            ;;
        sparcv9)
            mach=sparc
            lib_arch=sparcv9
            sysroot_sum=e249a7ef781b9b3297419bd014fa0574800703981d84e113d6af3a897a8b4ffc
            ;;
    esac

    # The Solaris 11.4 sysroot that rust-lang/rust builds its Solaris targets
    # against, so the libc symbols std links to are all there.
    #   https://github.com/rust-lang/rust/blob/main/src/ci/docker/scripts/solaris-toolchain.sh
    local sysroot_file="solaris-11.4.42.111.0-${mach}-sysroot-v2025-02-21.tar.bz2"
    curl --retry 3 -sSfL "https://ci-mirrors.rust-lang.org/rustc/${sysroot_file}" -O
    sha256sum --check <<<"${sysroot_sum}  ${sysroot_file}"
    tar -C "${td}/solaris" -xjf "${sysroot_file}"

    cd binutils-build
    ../binutils/configure \
        --target="${target}"
    make "-j$(nproc)"
    make install
    cd ..

    local destdir="/usr/local/${target}"
    mkdir "${destdir}/usr"
    cp -r "${td}/solaris/usr/include" "${destdir}/usr"
    mv "${td}/solaris/usr/lib/${lib_arch}"/* "${destdir}/lib"
    mv "${td}/solaris/lib/${lib_arch}"/* "${destdir}/lib"

    ln -s usr/include "${destdir}/sys-include"
    ln -s usr/include "${destdir}/include"

    # note: solaris2.10 is obsolete, so we can't upgrade to GCC 10 till then.
    # for gcc 9.4.0, need `--enable-obsolete`
    cd gcc-build
    ../gcc/configure \
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
        --disable-nls \
        --enable-languages=c,c++,fortran \
        --with-gnu-as \
        --with-gnu-ld \
        --target="${target}"
    make "-j$(nproc)"
    make install
    cd ..

    # Map old to new symbols. This file is picked up by linker flags in our tests.
    # - https://github.com/cross-rs/cross/issues/1599
    # - https://github.com/cross-rs/cross/issues/1730
    cat << '_EOF_' > compat.c
#include <unistd.h>

// Forward declare the existing DilOS functions
extern long __sysconf_xpg5(int);
extern int __xnet_socket(int, int, int);

// Create strong aliases that the Rust compiler can see
long __sysconf_xpg7(int name) {
    return __sysconf_xpg5(name);
}
int __xnet7_socket(int domain, int type, int protocol) {
    return __xnet_socket(domain, type, protocol);
}
_EOF_
    "${target}-gcc" -c compat.c -o /compat.o

    # clean up
    popd

    purge_packages

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
