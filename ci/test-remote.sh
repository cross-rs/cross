#!/usr/bin/env bash
# shellcheck disable=SC1091,SC1090

# test to see that remote docker support works.

set -x
set -eo pipefail

export CROSS_REMOTE=1
if [[ -z "${TARGET}" ]]; then
    export TARGET="aarch64-unknown-linux-gnu"
fi

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
. "${ci_dir}"/shared.sh
project_home=$(dirname "${ci_dir}")

main() {
    local err=

    retry cargo fetch
    cargo build
    export CROSS="${project_home}/target/debug/cross"
    export CROSS_UTIL="${project_home}/target/debug/cross-util"

    # if the create volume fails, ensure it exists.
    if ! err=$("${CROSS_UTIL}" volumes create 2>&1 >/dev/null); then
        if [[ "${err}" != *"already exists"* ]]; then
            echo "${err}"
            exit 1
        fi
    fi
    cross_test_cpp
    "${CROSS_UTIL}" volumes remove

    # ensure the data volume was removed.
    cross_test_cpp
}

cross_test_cpp() {
    local td=
    td="$(mktemp -d)"

    git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"

    pushd "${td}"
    retry cargo fetch
    "${CROSS}" run --target "${TARGET}"
    popd

    rm -rf "${td}"
}

main
