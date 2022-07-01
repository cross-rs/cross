#!/usr/bin/env bash
# shellcheck disable=SC1090,SC1091

set -x
set -euo pipefail

. lib.sh

main() {
    local project_dir="${1}"

    install_packages ca-certificates curl gcc libc6-dev

    cd "${project_dir}"
    curl --proto "=https" --tlsv1.2 --retry 3 -sSfL https://sh.rustup.rs | sh -s -- -y
    source "${HOME}"/.cargo/env
    cargo install --path . --locked

    purge_packages

    rm -rf "${0}"
}

main "${@}"
