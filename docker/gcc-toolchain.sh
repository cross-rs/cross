#!/bin/bash

set -x
set -euo pipefail
. lib.sh

main() {
    local target="${1}"
    local args="${2}"

    local gcc=8.3.0 \
          glibc=2.28 \
          binutils=2.31.1 \
          linux=5.13.2

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
    mkdir "${td}"/build-{binutils,gcc,glibc} /usr/"${target}"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/gcc/gcc-${gcc}/gcc-${gcc}.tar.gz" -O
    tar -xf "gcc-${gcc}.tar.gz"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/libc/glibc-${glibc}.tar.bz2" -O
    tar -xjf "glibc-${glibc}.tar.bz2"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/binutils/binutils-${binutils}.tar.bz2" -O
    tar -xjf "binutils-${binutils}.tar.bz2"

    curl --retry 3 -sSfL "https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-${linux}.tar.xz" -O
    tar -xJf "linux-${linux}.tar.xz"

    pushd "gcc-${gcc}"
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    popd

    pushd "build-binutils"
    ../binutils-${binutils}/configure \
        --prefix=/usr/"${target}" --target="${target}" \
        --disable-multilib \
        ${args}
    make -j$(nproc)
    make install
    popd

    pushd "build-gcc"
    ../gcc-${gcc}/configure \
        --prefix=/usr/"${target}" \
        --target="${target}" \
        --enable-languages=c,c++ \
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
        ${args}
    make -j$(nproc) all-gcc
    make install-gcc
    popd

    export PATH=/usr/"${target}"/bin:$PATH

    pushd "linux-${linux}"
    make \
        INSTALL_HDR_PATH=/usr/"${target}"/"${target}" \
        headers_install
    popd

    pushd "build-glibc"
    CC="${target}"-gcc ../glibc-${glibc}/configure \
        --prefix=/usr/"${target}"/"${target}" \
        --build="${MACHTYPE}" --host="${target}" --target="${target}" \
        --with-headers=/usr/"${target}"/"${target}"/include \
        --disable-multilib libc_cv_forced_unwind=yes \
        ${args}
    make install-bootstrap-headers=yes install-headers
    make -j$(nproc) csu/subdir_lib
    install csu/crt1.o csu/crti.o csu/crtn.o /usr/"${target}"/"${target}"/lib
    "${target}"-gcc -nostdlib -nostartfiles -shared -x c /dev/null -o /usr/"${target}"/"${target}"/lib/libc.so
    touch /usr/"${target}"/"${target}"/include/gnu/stubs.h
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