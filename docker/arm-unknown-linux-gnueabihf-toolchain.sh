#!/bin/bash

set -x
set -euo pipefail
. lib.sh

main() {
    local gcc_version=8.3.0 \
          glibc_version=2.28 \
          binutils_version=2.31.1 \
          linux_version=5.13.2

    install_packages \
        rsync \
        git \
        cmake \
        gdb \
        gdbserver \
        bzip2 \
        gawk \
        bison \
        python3

    local td
    td="$(mktemp -d)"

    pushd "${td}"
    mkdir "${td}"/build-{binutils,gcc,glibc} /usr/arm-linux-gnueabihf

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/gcc/gcc-${gcc_version}/gcc-${gcc_version}.tar.gz" -O
    tar -xf "gcc-${gcc_version}.tar.gz"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/libc/glibc-${glibc_version}.tar.bz2" -O
    tar -xjf "glibc-${glibc_version}.tar.bz2"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/binutils/binutils-${binutils_version}.tar.bz2" -O
    tar -xjf "binutils-${binutils_version}.tar.bz2"

    curl --retry 3 -sSfL "https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-${linux_version}.tar.xz" -O
    tar -xJf "linux-${linux_version}.tar.xz"

    pushd "gcc-${gcc_version}"
    contrib/download_prerequisites && rm *.tar.*
    popd

    pushd "build-binutils"
    ../binutils-${binutils_version}/configure \
        --prefix=/usr/arm-linux-gnueabihf --target=arm-linux-gnueabihf \
        --with-arch=armv6 --with-fpu=vfp --with-float=hard \
        --disable-multilib
    make -j$(nproc)
    make install
    popd

    pushd "build-gcc"
    ../gcc-${gcc_version}/configure \
        --prefix=/usr/arm-linux-gnueabihf \
        --target=arm-linux-gnueabihf \
        --enable-languages=c,c++ \
        --with-arch=armv6 --with-fpu=vfp --with-float=hard \
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
        --disable-nls
    make -j$(nproc) all-gcc
    make install-gcc
    popd

    export PATH=/usr/arm-linux-gnueabihf/bin:$PATH

    pushd "linux-${linux_version}"
    export KERNEL=kernel7
    make \
        ARCH=arm \
        INSTALL_HDR_PATH=/usr/arm-linux-gnueabihf/arm-linux-gnueabihf \
        headers_install
    popd

    pushd "build-glibc"
    CC=arm-linux-gnueabihf-gcc ../glibc-${glibc_version}/configure \
        --prefix=/usr/arm-linux-gnueabihf/arm-linux-gnueabihf \
        --build=$MACHTYPE --host=arm-linux-gnueabihf --target=arm-linux-gnueabihf \
        --with-arch=armv6 --with-fpu=vfp --with-float=hard \
        --with-headers=/usr/arm-linux-gnueabihf/arm-linux-gnueabihf/include \
        --disable-multilib libc_cv_forced_unwind=yes    
    make install-bootstrap-headers=yes install-headers
    make -j$(nproc) csu/subdir_lib
    install csu/crt1.o csu/crti.o csu/crtn.o /usr/arm-linux-gnueabihf/arm-linux-gnueabihf/lib
    arm-linux-gnueabihf-gcc -nostdlib -nostartfiles -shared -x c /dev/null -o /usr/arm-linux-gnueabihf/arm-linux-gnueabihf/lib/libc.so
    touch /usr/arm-linux-gnueabihf/arm-linux-gnueabihf/include/gnu/stubs.h
    popd

    pushd "build-gcc"
    make -j$(nproc) all-target-libgcc
    make install-target-libgcc
    popd

    pushd "build-glibc"
    make -j$(nproc)
    make install
    popd

    pushd "build-gcc"
    make -j$(nproc)
    make install
    popd

    rm -rf "${td}"
}

main "${@}"