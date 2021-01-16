#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local arch="${1}"

    local base_release=12.1 \
          binutils=2.32 \
          gcc=6.4.0 \
          target="${arch}-unknown-freebsd12"

    local dependencies=(
        ca-certificates
        curl
        g++
        make
        wget
        xz-utils
    )

    apt-get update
    local purge_list=()
    for dep in "${dependencies[@]}"; do
        if ! dpkg -L "${dep}"; then
            apt-get install --no-install-recommends --assume-yes "${dep}"
            purge_list+=( "${dep}" )
        fi
    done

    local td
    td="$(mktemp -d)"

    mkdir "${td}"/{binutils,gcc}{,-build} "${td}/freebsd"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/binutils/binutils-${binutils}.tar.gz" -O
    tar -C "${td}/binutils" --strip-components=1 -xf "binutils-${binutils}.tar.gz"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/gcc/gcc-${gcc}/gcc-${gcc}.tar.gz" -O
    tar -C "${td}/gcc" --strip-components=1 -xf "gcc-${gcc}.tar.gz"

    pushd "${td}"

    cd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    cd ..

    local bsd_arch=
    case "${arch}" in
        x86_64)
            bsd_arch=amd64
            ;;
        i686)
            bsd_arch=i386
            ;;
    esac

    curl --retry 3 -sSfL "http://ftp.freebsd.org/pub/FreeBSD/releases/${bsd_arch}/${base_release}-RELEASE/base.txz" -O
    tar -C "${td}/freebsd" -xJf base.txz ./usr/include ./usr/lib ./lib

    cd binutils-build
    ../binutils/configure \
        --target="${target}"
    make "-j$(nproc)"
    make install
    cd ..

    local destdir="/usr/local/${target}"
    cp -r "${td}/freebsd/usr/include" "${destdir}"
    cp "${td}/freebsd/lib/libc.so.7" "${destdir}/lib"
    cp "${td}/freebsd/lib/libm.so.5" "${destdir}/lib"
    cp "${td}/freebsd/lib/libthr.so.3" "${destdir}/lib"
    cp "${td}/freebsd/lib/libutil.so.9" "${destdir}/lib"
    cp "${td}/freebsd/lib/libssp.so.0" "${destdir}/lib"
    cp "${td}/freebsd/usr/lib/libc++.so.1" "${destdir}/lib"
    cp "${td}/freebsd/usr/lib/libc++.a" "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/lib{c,util,m,ssp,ssp_nonshared}.a "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/lib{rt,execinfo}.so.1 "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/{crt1,Scrt1,crti,crtn}.o "${destdir}/lib"

    ln -s libc.so.7 "${destdir}/lib/libc.so"
    ln -s libc++.so.1 "${destdir}/lib/libc++.so"
    ln -s libexecinfo.so.1 "${destdir}/lib/libexecinfo.so"
    ln -s libm.so.5 "${destdir}/lib/libm.so"
    ln -s librt.so.1 "${destdir}/lib/librt.so"
    ln -s libutil.so.9 "${destdir}/lib/libutil.so"
    ln -s libthr.so.3 "${destdir}/lib/libpthread.so"
    ln -s libssp.so.0 "${destdir}/lib/libssp.so"

    cd gcc-build
    ../gcc/configure \
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
        --disable-nls \
        --enable-languages=c,c++ \
        --target="${target}"
    make "-j$(nproc)"
    make install
    cd ..

    # clean up
    popd

    if (( ${#purge_list[@]} )); then
      apt-get purge --assume-yes --auto-remove "${purge_list[@]}"
    fi

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
