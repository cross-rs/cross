#!/usr/bin/env bash

set -eo pipefail

PATH=$PATH:/rust/bin

main() {
    local td
    td="$(mktemp -d)"

    pushd "${td}" >/dev/null
    cargo init --bin hello 1>&2
    cd hello
    cargo build --target "${TARGET}" "${@}" 1>&2
    readelf -A "target/${TARGET}/debug/hello"

    popd >/dev/null
}

main "${@}"
