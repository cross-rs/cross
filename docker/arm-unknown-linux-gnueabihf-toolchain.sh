#!/bin/bash

. lib.sh

main() {
    local gcc_version=gcc-8.3.0
    local glibc_version=glibc-2.28
    local binutils_version=binutils-2.31.1

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

    cd /tmp

    curl -fL https://ftp.gnu.org/gnu/gcc/${gcc_version}/${gcc_version}.tar.gz -O
    tar xf ${gcc_version}.tar.gz
    rm ${gcc_version}.tar.gz

    curl -fL https://ftp.gnu.org/gnu/libc/${glibc_version}.tar.bz2 -O
    tar xjf ${glibc_version}.tar.bz2
    rm ${glibc_version}.tar.bz2

    curl -fL https://ftp.gnu.org/gnu/binutils/${binutils_version}.tar.bz2 -O
    tar xjf ${binutils_version}.tar.bz2
    rm ${binutils_version}.tar.bz2

    cd ${gcc_version}
    contrib/download_prerequisites && rm *.tar.*

    mkdir -p /usr/arm-linux-gnueabihf && cd /usr/arm-linux-gnueabihf

    mkdir /tmp/build-binutils && cd /tmp/build-binutils
    ../${binutils_version}/configure \
        --prefix=/usr/arm-linux-gnueabihf --target=arm-linux-gnueabihf \
        --with-arch=armv6 --with-fpu=vfp --with-float=hard \
        --disable-multilib
    make -j$(nproc) && make install

    mkdir /tmp/build-gcc && cd /tmp/build-gcc
    ../${gcc_version}/configure \
        --prefix=/usr/arm-linux-gnueabihf \
        --target=arm-linux-gnueabihf \
        --enable-languages=c,c++,fortran \
        --with-arch=armv6 --with-fpu=vfp --with-float=hard \
        --disable-multilib
    make -j$(nproc) all-gcc && make install-gcc

    export PATH=/usr/arm-linux-gnueabihf/bin:$PATH

    cd /tmp

    git clone --depth=1 https://github.com/raspberrypi/linux && cd linux
    export KERNEL=kernel7
    make ARCH=arm INSTALL_HDR_PATH=/usr/arm-linux-gnueabihf/arm-linux-gnueabihf headers_install

    mkdir /tmp/build-glibc && cd /tmp/build-glibc

    CC=arm-linux-gnueabihf-gcc ../${glibc_version}/configure \
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

    cd /tmp/build-gcc
    make -j$(nproc) all-target-libgcc && make install-target-libgcc

    cd /tmp/build-glibc
    make -j$(nproc) && make install

    cd /tmp/build-gcc
    make -j$(nproc) && make install

    rm -rf /tmp/*
}

main "${@}"