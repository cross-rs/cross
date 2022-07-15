#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. freebsd-common.sh

export PACKAGESITE=/opt/freebsd-packagesite/packagesite.yaml
export PKG_SOURCE="https://pkg.freebsd.org/FreeBSD:${BSD_MAJOR}:${BSD_ARCH}/quarterly"
export TARGET="${ARCH}-unknown-freebsd${BSD_MAJOR}"

setup_packagesite() {
    apt-get update && apt-get install --assume-yes --no-install-recommends \
        curl \
        jq \
        xz-utils

    mkdir /opt/freebsd-packagesite
    curl --retry 3 -sSfL "${PKG_SOURCE}/packagesite.txz" -O
    tar -C /opt/freebsd-packagesite -xJf packagesite.txz

    rm packagesite.txz
}

install_freebsd_package() {
    local name
    local path
    local pkg
    local td
    local destdir="/usr/local/${TARGET}"

    td="$(mktemp -d)"
    pushd "${td}"

    for name in "${@}"; do
        path=$(jq -c '. | select ( .name == "'"${name}"'" ) | .repopath' "${PACKAGESITE}")
        if [[ -z "${path}" ]]; then
            echo "Unable to find package ${name}" >&2
            exit 1
        fi
        path=${path//'"'/}
        pkg=$(basename "${path}")

        mkdir "${td}"/package
        curl --retry 3 -sSfL "${PKG_SOURCE}/${path}" -O
        tar -C "${td}/package" -xJf "${pkg}"
        cp -r "${td}/package/usr/local"/* "${destdir}"/

        rm "${td:?}/${pkg}"
        rm -rf "${td:?}/package"
    done

    # clean up
    popd
    rm -rf "${td:?}"
}
