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

main() {
    local err=

    retry cargo fetch
    cargo build
    export CROSS="${PROJECT_HOME}/target/debug/cross"
    export CROSS_UTIL="${PROJECT_HOME}/target/debug/cross-util"

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
    td="$(mkcargotemp -d)"

    git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"

    pushd "${td}"
    retry cargo fetch
    "${CROSS}" run --target "${TARGET}" | grep "Hello, world!"
    sed -i 's/Hello, world/Hello, test/g' hellopp.cc
    "${CROSS}" run --target "${TARGET}" | grep "Hello, test!"
    popd

    rm -rf "${td}"
}

main
