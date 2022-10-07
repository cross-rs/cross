#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local arch
    arch=$(docker_to_qemu_arch "${TARGETARCH}")
    /qemu.sh "${arch}" softmmu
    rm "${0}"
}

main "${@}"
