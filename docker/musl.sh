#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

hide_output() {
    set +x
    trap "
        echo 'ERROR: An error was encountered with the build.'
        cat /tmp/build.log
        exit 1
    " ERR
    bash -c 'while true; do sleep 30; echo $(date) - building ...; done' &
    PING_LOOP_PID=$!
    "${@}" &> /tmp/build.log
    trap - ERR
    kill "${PING_LOOP_PID}"
    set -x
}

main() {
    local version=fe91582

    install_packages ca-certificates curl build-essential

    local td
    td="$(mktemp -d)"

    pushd "${td}"
    curl --retry 3 -sSfL "https://github.com/richfelker/musl-cross-make/archive/${version}.tar.gz" -O
    tar --strip-components=1 -xzf "${version}.tar.gz"

    # Don't depend on the mirrors of sabotage linux that musl-cross-make uses.
    local linux_headers_site=https://ci-mirrors.rust-lang.org/rustc/sabotage-linux-tarballs
    local linux_ver=headers-4.19.88

    # alpine GCC is built with `--enable-default-pie`, so we want to
    # ensure we use that. we want support for shared runtimes except for
    # libstdc++, however, the only way to do that is to simply remove
    # the shared libraries later. on alpine, binaries use static-pie
    # linked, so our behavior has maximum portability, and is consistent
    # with popular musl distros.
    hide_output make install "-j$(nproc)" \
        GCC_VER=9.2.0 \
        MUSL_VER=1.2.3 \
        BINUTILS_VER=2.33.1 \
        DL_CMD='curl --retry 3 -sSfL -C - -o' \
        LINUX_HEADERS_SITE="${linux_headers_site}" \
        LINUX_VER="${linux_ver}" \
        OUTPUT=/usr/local/ \
        "GCC_CONFIG += --enable-default-pie --enable-languages=c,c++,fortran" \
        "${@}"

    purge_packages

    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
