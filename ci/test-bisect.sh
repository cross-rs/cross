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


main() {
    local td=
    local err=

    retry cargo fetch
    cargo build
    cargo install cargo-bisect-rustc --debug
    export CROSS="${PROJECT_HOME}/target/debug/cross"

    td="$(mktemp -d)"
    git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"

    pushd "${td}"
    retry cargo fetch
    # shellcheck disable=SC2016
    echo '#!/usr/bin/env bash
export CROSS_CUSTOM_TOOLCHAIN=1
"${CROSS}" run --target '"${TARGET}"'
cargo -V | grep 2022-06
' > bisect.sh
    chmod +x bisect.sh

    if ! err=$(cargo-bisect-rustc --start 2022-07-01 --end 2022-07-03 --script=./bisect.sh --target "${TARGET}" 2>&1); then
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
