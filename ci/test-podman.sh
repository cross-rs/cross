#!/usr/bin/env bash
# shellcheck disable=SC1091,SC1090

# test to see that running and building images with podman works.

set -x
set -eo pipefail

export CROSS_CONTAINER_ENGINE=podman
if [[ -z "${TARGET}" ]]; then
    export TARGET="aarch64-unknown-linux-gnu"
fi

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
. "${ci_dir}"/shared.sh

main() {
    local td=
    local parent=
    local target="${TARGET}"

    retry cargo fetch
    cargo build
    CROSS=$(binary_path cross "${PROJECT_HOME}" debug)
    export CROSS

    td="$(mkcargotemp -d)"
    parent=$(dirname "${td}")
    pushd "${td}"
    cargo init --bin --name "hello" .

    echo '[build]
pre-build = ["apt-get update"]' > "${parent}/Cross.toml"

    CROSS_CONTAINER_ENGINE="${CROSS_ENGINE}" "${CROSS}" build --target "${target}" --verbose

    popd
    rm -rf "${td}"
}

main
