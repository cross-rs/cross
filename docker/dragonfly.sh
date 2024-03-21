#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local nproc=
    local binutils=2.32 \
        dragonfly=6.0.1_REL \
        gcc=10.3.0 \
        target=x86_64-unknown-dragonfly
    if [[ $# != "0" ]]; then
        nproc="${1}"
    fi

    install_packages libarchive-tools \
        bzip2 \
        ca-certificates \
        curl \
        g++ \
        make \
        patch \
        wget \
        xz-utils

    local td
    td="$(mktemp -d)"

    pushd "${td}"
    mkdir "${td}"/{binutils,gcc}{,-build} "${td}/dragonfly"

    download_binutils "${binutils}" "bz2"
    tar -C "${td}/binutils" --strip-components=1 -xjf "binutils-${binutils}.tar.bz2"

    download_gcc "${gcc}" "gz"
    tar -C "${td}/gcc" --strip-components=1 -xf "gcc-${gcc}.tar.gz"

    cd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    patch libstdc++-v3/configure <<'EOF'
47159c47159
<   *-freebsd*)
---
>   *-freebsd* | *-dragonfly*)
EOF
    cd ..

    local mirrors=(
        "https://mirror-master.dragonflybsd.org/iso-images"
        "https://avalon.dragonflybsd.org/iso-images/"
    )
    download_mirrors "" "dfly-x86_64-${dragonfly}.iso.bz2" "${mirrors[@]}"
    bzcat "dfly-x86_64-${dragonfly}.iso.bz2" | bsdtar xf - -C "${td}/dragonfly" ./usr/include ./usr/lib ./lib

    cd binutils-build
    ../binutils/configure \
        --target="${target}"
    make "-j${nproc}"
    make install
    cd ..

    # note: shell expansions can't be quoted
    local destdir="/usr/local/${target}"
    cp -r "${td}/dragonfly/usr/include" "${destdir}"/
    cp "${td}/dragonfly/lib/libc.so.8" "${destdir}/lib"
    cp "${td}/dragonfly/lib/libm.so.4" "${destdir}/lib"
    cp "${td}/dragonfly/lib/libutil.so.4" "${destdir}/lib"
    cp "${td}/dragonfly/usr/lib/libexecinfo.so.1" "${destdir}/lib"
    cp "${td}/dragonfly/usr/lib/libpthread.so" "${destdir}/lib/libpthread.so"
    cp "${td}/dragonfly/usr/lib/librt.so.0" "${destdir}/lib"
    cp "${td}"/dragonfly/usr/lib/lib{c,m,util,kvm}.a "${destdir}/lib"
    cp "${td}/dragonfly/usr/lib/thread/libthread_xu.so.2" "${destdir}/lib/libpthread.so.0"
    cp "${td}"/dragonfly/usr/lib/{crt1,Scrt1,crti,crtn}.o "${destdir}/lib/"

    ln -s libc.so.8 "${destdir}/lib/libc.so"
    ln -s libexecinfo.so.1 "${destdir}/lib/libexecinfo.so"
    ln -s libm.so.4 "${destdir}/lib/libm.so"
    ln -s librt.so.0 "${destdir}/lib/librt.so"
    ln -s libutil.so.4 "${destdir}/lib/libutil.so"

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
        --target="${target}"
    make "-j${nproc}"
    make install
    cd ..

    # rust incorrectly adds link args to libgcc_pic, which is no longer
    # a present target, and it should link to libgcc_s.
    # https://github.com/rust-lang/rust/blob/60361f2/library/unwind/build.rs#L23-L38
    ln -s "${destdir}"/lib/libgcc_s.so "${destdir}"/lib/libgcc_pic.so

    # clean up
    popd

    purge_packages

    # store the version info for the dragonfly release
    echo "${dragonfly}" > /opt/dragonfly-version

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
