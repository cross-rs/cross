#!/usr/bin/env bash
# shellcheck disable=SC1091,SC1090

# test to see that custom toolchains work

set -x
set -eo pipefail

if [[ -z "${TARGET}" ]]; then
    export TARGET="aarch64-unknown-linux-gnu"
fi

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
. "${ci_dir}"/shared.sh
project_home=$(dirname "${ci_dir}")

main() {
    local td=
    local err=

    retry cargo fetch
    cargo build
    cargo install cargo-bisect-rustc --debug
    export CROSS="${project_home}/target/debug/cross"

    td="$(mktemp -d)"
    git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"

    pushd "${td}"
    retry cargo fetch
    # shellcheck disable=SC2016
    echo '#!/usr/bin/env bash
export CROSS_CUSTOM_TOOLCHAIN=1
exec "${CROSS}" run --target '"${TARGET}" > bisect.sh
    chmod +x bisect.sh

    if ! err=$(cargo bisect-rustc --script=./bisect.sh --target "${TARGET}" 2>&1 >/dev/null); then
        if [[ "${err}" != *"does not reproduce the regression"* ]]; then
            echo "${err}"
            exit 1
        fi
    else
        echo "should have failed, instead succeeded" 1>&2
        exit 1
    fi
    popd

    rm -rf "${td}"
}

main
