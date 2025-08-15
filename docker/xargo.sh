#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    install_packages ca-certificates curl

    export RUSTUP_HOME=/tmp/rustup
    export CARGO_HOME=/tmp/cargo

    curl --retry 3 -sSfL https://sh.rustup.rs -o rustup-init.sh
    # xargo does not always build with the most recent default version of rust,
    # and it may be desirable to explicitly select a `rust` version anyway,
    # so allow specifying that. If unspecified or set to `latest`, just use
    # the rustup default.
    if [[ $# -gt 0 ]] && [[ -n "${1}" ]] && [[ "${1}" != "latest" ]]; then
        sh rustup-init.sh -y --no-modify-path --profile minimal --default-toolchain="${1}"
    else
        sh rustup-init.sh -y --no-modify-path --profile minimal
    fi
    rm rustup-init.sh

    PATH="${CARGO_HOME}/bin:${PATH}" cargo install xargo --root /usr/local

    rm -r "${RUSTUP_HOME}" "${CARGO_HOME}"

    purge_packages

    rm "${0}"
}

main "${@}"
