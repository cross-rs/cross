#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local arch="${1}"

    local binutils=2.28.1 \
          gcc=8.4.0 \
          target="${arch}-sun-solaris2.10"

    install_packages bzip2 \
        ca-certificates \
        curl \
        dirmngr \
        g++ \
        gpg-agent \
        make \
        patch \
        software-properties-common \
        wget \
        xz-utils

    local td
    td="$(mktemp -d)"
    pushd "${td}"

    mkdir "${td}"/{binutils,gcc}{,-build} "${td}/solaris"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/binutils/binutils-${binutils}.tar.xz" -O
    tar -C "${td}/binutils" --strip-components=1 -xJf "binutils-${binutils}.tar.xz"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/gcc/gcc-${gcc}/gcc-${gcc}.tar.xz" -O
    tar -C "${td}/gcc" --strip-components=1 -xJf "gcc-${gcc}.tar.xz"

    cd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    cd ..

    local apt_arch=
    local lib_arch=
    case "${arch}" in
        x86_64)
            apt_arch=solaris-i386
            lib_arch=amd64
            ;;
        sparcv9)
            apt_arch=solaris-sparc
            lib_arch=sparcv9
            ;;
    esac

    apt-key adv --batch --yes --keyserver keyserver.ubuntu.com --recv-keys 74DA7924C5513486
    add-apt-repository -y 'deb http://apt.dilos.org/dilos dilos2-testing main'
    dpkg --add-architecture "${apt_arch}"
    apt-get update
    # shellcheck disable=SC2046
    apt-get download $(apt-cache depends --recurse --no-replaces \
      "libc:${apt_arch}"            \
      "liblgrp-dev:${apt_arch}"     \
      "liblgrp:${apt_arch}"         \
      "libm-dev:${apt_arch}"        \
      "libpthread:${apt_arch}"      \
      "libresolv:${apt_arch}"       \
      "librt:${apt_arch}"           \
      "libsendfile-dev:${apt_arch}" \
      "libsendfile:${apt_arch}"     \
      "libsocket:${apt_arch}"       \
      "system-crt:${apt_arch}"      \
      "system-header:${apt_arch}"   \
      | grep "^\w")

    for deb in *"${apt_arch}.deb"; do
      dpkg -x "${deb}" "${td}/solaris"
    done

    cd binutils-build
    ../binutils/configure \
        --target="${target}"
    make "-j$(nproc)"
    make install
    cd ..

    # Remove Solaris 11 functions that are optionally used by libbacktrace.
    # This is for Solaris 10 compatibility.
    rm solaris/usr/include/link.h

    patch -p0  << 'EOF'
--- solaris/usr/include/string.h
+++ solaris/usr/include/string10.h
@@ -93 +92,0 @@
-extern size_t strnlen(const char *, size_t);
EOF

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
        --enable-languages=c,c++ \
        --with-gnu-as \
        --with-gnu-ld \
        --target="${target}"
    make "-j$(nproc)"
    make install
    cd ..

    # clean up
    popd

    purge_packages

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
