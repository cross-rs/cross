#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local arch="${1}"

    local sqlite_ver=3.34.0,1 \
          openssl_ver=1.1.1j,1 \
          target="${arch}-unknown-freebsd12"

    local td
    td="$(mktemp -d)"

    mkdir "${td}"/{openssl,sqlite}

    pushd "${td}"

    local bsd_arch=
    case "${arch}" in
        x86_64)
            bsd_arch=amd64
            ;;
        i686)
            bsd_arch=i386
            ;;
    esac

    # Adding openssl lib
    curl --retry 3 -sSfL "https://pkg.freebsd.org/FreeBSD:12:${bsd_arch}/quarterly/All/openssl-${openssl_ver}.txz" -O
    tar -C "${td}/openssl" -xJf openssl-${openssl_ver}.txz /usr/local/lib /usr/local/include/

    # Adding sqlite3
    curl --retry 3 -sSfL "https://pkg.freebsd.org/FreeBSD:12:${bsd_arch}/quarterly/All/sqlite3-${sqlite_ver}.txz" -O
    tar -C "${td}/sqlite" -xJf sqlite3-${sqlite_ver}.txz /usr/local/lib

    # Copy the linked library
    local destdir="/usr/local/${target}"
    cp -r "${td}/openssl/usr/local/include" "${destdir}"
    cp "${td}/openssl/usr/local/lib"/lib{crypto,ssl}.a "${destdir}/lib"
    cp "${td}/openssl/usr/local/lib"/lib{crypto,ssl}.so.11 "${destdir}/lib"
    cp "${td}/openssl/usr/local/lib"/lib{crypto,ssl}.so "${destdir}/lib"
    cp "${td}/sqlite/usr/local/lib/libsqlite3.a" "${destdir}/lib"
    cp "${td}/sqlite/usr/local/lib/libsqlite3.so.0.8.6" "${destdir}/lib"
    cp "${td}/sqlite/usr/local/lib/libsqlite3.so" "${destdir}/lib"
    cp "${td}/sqlite/usr/local/lib/libsqlite3.so.0" "${destdir}/lib"

    # clean up
    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
