#!/usr/bin/env bash

set -x
set -euo pipefail

export ARCH="${1}"
# shellcheck disable=SC1091
. lib.sh
# shellcheck disable=SC1091
. freebsd-common.sh

main() {
    local pkg_source="https://pkg.freebsd.org/FreeBSD:${BSD_MAJOR}:${BSD_ARCH}/quarterly"
    install_packages curl jq xz-utils

    local td
    td="$(mktemp -d)"

    mkdir "${td}"/{openssl,sqlite,packagesite}

    pushd "${td}"

    curl --retry 3 -sSfL "${pkg_source}/packagesite.txz" -O
    tar -C "${td}/packagesite" -xJf packagesite.txz
    local openssl_path
    local sqlite_path
    local openssl_pkg
    local sqlite_pkg
    openssl_path=$(jq -c '. | select ( .name == "openssl" ) | .repopath' "${td}/packagesite/packagesite.yaml")
    sqlite_path=$(jq -c '. | select ( .name == "sqlite3" ) | .repopath' "${td}/packagesite/packagesite.yaml")
    openssl_path=${openssl_path//'"'/}
    sqlite_path=${sqlite_path//'"'/}
    openssl_pkg=$(basename "${openssl_path}")
    sqlite_pkg=$(basename "${sqlite_path}")

    local target="${ARCH}-unknown-freebsd${BSD_MAJOR}"

    # Adding openssl lib
    curl --retry 3 -sSfL "${pkg_source}/${openssl_path}" -O
    tar -C "${td}/openssl" -xJf "${openssl_pkg}" /usr/local/lib /usr/local/include/

    # Adding sqlite3
    curl --retry 3 -sSfL "${pkg_source}/${sqlite_path}" -O
    tar -C "${td}/sqlite" -xJf "${sqlite_pkg}" /usr/local/lib

    # Copy the linked library
    local destdir="/usr/local/${target}"
    cp -r "${td}/openssl/usr/local/include" "${destdir}"
    cp "${td}/openssl/usr/local/lib"/lib{crypto,ssl}.a "${destdir}/lib"
    cp "${td}/openssl/usr/local/lib"/lib{crypto,ssl}.so* "${destdir}/lib"
    cp "${td}/sqlite/usr/local/lib"/libsqlite3.so* "${destdir}/lib"

    purge_packages

    # clean up
    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
