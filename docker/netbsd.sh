#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local binutils=2.36.1 \
        gcc=9.4.0 \
        target=x86_64-unknown-netbsd

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

    mkdir "${td}"/{binutils,gcc}{,-build} "${td}/netbsd"

    download_binutils "${binutils}" "bz2"
    tar -C "${td}/binutils" --strip-components=1 -xjf "binutils-${binutils}.tar.bz2"

    download_gcc "${gcc}" "xz"
    tar -C "${td}/gcc" --strip-components=1 -xJf "gcc-${gcc}.tar.xz"

    pushd "${td}"

    pushd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    local patches=(
        https://ftp.netbsd.org/pub/pkgsrc/current/pkgsrc/lang/gcc9/patches/patch-libstdc++-v3_config_os_bsd_netbsd_ctype__configure__char.cc
        https://ftp.netbsd.org/pub/pkgsrc/current/pkgsrc/lang/gcc9/patches/patch-libstdc++-v3_config_os_bsd_netbsd_ctype__base.h
        https://ftp.netbsd.org/pub/pkgsrc/current/pkgsrc/lang/gcc8/patches/patch-libgfortran_io_io.h
    )

    local patch
    for patch in "${patches[@]}"; do
        local patch_file
        patch_file="$(mktemp)"
        curl --retry 3 -sSfL "${patch}" -o "${patch_file}"
        patch -Np0 < "${patch_file}"
        rm "${patch_file}"
    done
    popd

    local mirrors=(
        "ftp://ftp.netbsd.org"
        "https://cdn.NetBSD.org"
    )
    download_mirrors "pub/NetBSD/NetBSD-9.2/amd64/binary/sets" "base.tar.xz" "${mirrors[@]}"
    tar -C "${td}/netbsd" -xJf base.tar.xz ./usr/include ./usr/lib ./lib

    download_mirrors "pub/NetBSD/NetBSD-9.2/amd64/binary/sets" "comp.tar.xz" "${mirrors[@]}"
    tar -C "${td}/netbsd" -xJf comp.tar.xz ./usr/include ./usr/lib

    pushd binutils-build
    ../binutils/configure \
        --target="${target}"
    make "-j$(nproc)"
    make install
    popd

    local destdir="/usr/local/${target}"
    cp -r "${td}/netbsd/usr/include" "${destdir}"/
    ls -all "${td}/netbsd/usr/lib"
    cp "${td}/netbsd/lib/libc.so.12.213" "${destdir}/lib"
    cp "${td}/netbsd/lib/libm.so.0.12" "${destdir}/lib"
    cp "${td}/netbsd/lib/libutil.so.7.24" "${destdir}/lib"
    cp "${td}/netbsd/lib/libpthread.so.1.4" "${destdir}/lib"
    cp "${td}/netbsd/usr/lib/librt.so.1.1" "${destdir}/lib"
    cp "${td}/netbsd/usr/lib"/lib{c,m,pthread}{,_p}.a "${destdir}/lib"
    cp "${td}/netbsd/usr/lib"/libexecinfo.so "${destdir}/lib"
    cp "${td}/netbsd/usr/lib"/{crt0,crti,crtn,crtbeginS,crtendS,crtbegin,crtend,gcrt0}.o "${destdir}/lib"

    ln -s libc.so.12.213 "${destdir}/lib/libc.so"
    ln -s libc.so.12.213 "${destdir}/lib/libc.so.12"
    ln -s libm.so.0.12 "${destdir}/lib/libm.so"
    ln -s libm.so.0.12 "${destdir}/lib/libm.so.0"
    ln -s libpthread.so.1.4 "${destdir}/lib/libpthread.so"
    ln -s libpthread.so.1.4 "${destdir}/lib/libpthread.so.1"
    ln -s librt.so.1.1 "${destdir}/lib/librt.so"
    ln -s libutil.so.7.24 "${destdir}/lib/libutil.so"
    ln -s libutil.so.7.24 "${destdir}/lib/libutil.so.7"

    pushd gcc-build
    # remove the environment variables after bumping the gcc version to 11.
    target_configargs="ac_cv_func_newlocale=no ac_cv_func_freelocale=no ac_cv_func_uselocale=no" ../gcc/configure \
        --disable-libada \
        --disable-libcilkrt \
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
        --target="${target}"
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
