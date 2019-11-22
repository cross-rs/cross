#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local dependencies=(
        ca-certificates
        curl
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    export RUSTUP_HOME=/tmp/rustup
    export CARGO_HOME=/tmp/cargo

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup-init.sh
    sh rustup-init.sh -y --no-modify-path
    rm rustup-init.sh

    PATH="${CARGO_HOME}/bin:${PATH}" cargo install xargo --root /usr

    rm -r "${RUSTUP_HOME}" "${CARGO_HOME}"

    apt-get purge --auto-remove -y ${purge_list[@]}
    rm $0
}

main "${@}"
