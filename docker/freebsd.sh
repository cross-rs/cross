#!/usr/bin/env bash

set -x
set -euo pipefail

export ARCH="${1}"
# shellcheck disable=SC1091
. freebsd-common.sh
# shellcheck disable=SC1091
. lib.sh

max_freebsd() {
    local best=
    local minor=0
    local version=
    local release_major=
    local release_minor=
    for release in "${@}"; do
        version=$(echo "${release}" | cut -d '-' -f 1)
        release_major=$(echo "${version}"| cut -d '.' -f 1)
        release_minor=$(echo "${version}"| cut -d '.' -f 2)
        if [ "${release_major}" == "${BSD_MAJOR}" ] && [ "${release_minor}" -gt "${minor}" ]; then
            best="${release}"
            minor="${release_minor}"
        fi
    done
    if [[ -z "$best" ]]; then
        echo "Could not find best release for FreeBSD ${BSD_MAJOR}." 1>&2
        exit 1
    fi
    echo "${best}"
}

latest_freebsd() {
    local dirs
    local releases
    local max_release

    dirs=$(curl --silent --list-only "${BSD_HOME}/${BSD_ARCH}/" | grep RELEASE)
    read -r -a releases <<< "${dirs[@]}"
    max_release=$(max_freebsd "${releases[@]}")

    echo "${max_release//-RELEASE/}"
}

base_release="$(latest_freebsd)"
bsd_ftp="${BSD_HOME}/${BSD_ARCH}/${base_release}-RELEASE"
bsd_http="http://${bsd_ftp}"

main() {
    local binutils=2.32 \
          gcc=6.4.0 \
          target="${ARCH}-unknown-freebsd${BSD_MAJOR}"

    install_packages ca-certificates \
        curl \
        g++ \
        make \
        wget \
        xz-utils

    local td
    td="$(mktemp -d)"
    pushd "${td}"

    mkdir "${td}"/{binutils,gcc}{,-build} "${td}/freebsd"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/binutils/binutils-${binutils}.tar.gz" -O
    tar -C "${td}/binutils" --strip-components=1 -xf "binutils-${binutils}.tar.gz"

    curl --retry 3 -sSfL "https://ftp.gnu.org/gnu/gcc/gcc-${gcc}/gcc-${gcc}.tar.gz" -O
    tar -C "${td}/gcc" --strip-components=1 -xf "gcc-${gcc}.tar.gz"

    cd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    cd ..

    curl --retry 3 -sSfL "${bsd_http}/base.txz" -O
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
    cp "${td}/freebsd/lib/libkvm.so.7" "${destdir}/lib"
    cp "${td}/freebsd/lib/libthr.so.3" "${destdir}/lib"
    cp "${td}/freebsd/lib/libutil.so.9" "${destdir}/lib"
    cp "${td}/freebsd/lib/libdevstat.so.7" "${destdir}/lib"
    cp "${td}/freebsd/usr/lib/libc++.so.1" "${destdir}/lib"
    cp "${td}/freebsd/usr/lib/libc++.a" "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/lib{c,util,m,ssp_nonshared}.a "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/lib{rt,execinfo,procstat}.so.1 "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/{crt1,Scrt1,crti,crtn}.o "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/libkvm.a "${destdir}/lib"

    ln -s libc.so.7 "${destdir}/lib/libc.so"
    ln -s libc++.so.1 "${destdir}/lib/libc++.so"
    ln -s libexecinfo.so.1 "${destdir}/lib/libexecinfo.so"
    ln -s libprocstat.so.1 "${destdir}/lib/libprocstat.so"
    ln -s libm.so.5 "${destdir}/lib/libm.so"
    ln -s librt.so.1 "${destdir}/lib/librt.so"
    ln -s libutil.so.9 "${destdir}/lib/libutil.so"
    ln -s libthr.so.3 "${destdir}/lib/libpthread.so"
    ln -s libdevstat.so.7 "${destdir}/lib/libdevstat.so"
    ln -s libkvm.so.7 "${destdir}/lib/libkvm.so"

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

    purge_packages

    # store the version info for the FreeBSD release
    bsd_revision=$(curl --retry 3 -sSfL "${bsd_http}/REVISION")
    echo "${base_release} (${bsd_revision})" > /opt/freebsd-version

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
