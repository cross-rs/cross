#!/usr/bin/env bash

set -x
set -eo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local arch
    arch=$(docker_to_linux_arch "${TARGETARCH}" "${TARGETVARIANT}")
    /linux-image.sh "${arch}"
    rm "${0}"
}

main "${@}"
