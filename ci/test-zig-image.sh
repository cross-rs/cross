#!/usr/bin/env bash
# shellcheck disable=SC2086,SC1091,SC1090

set -x
set -eo pipefail

# NOTE: "${@}" is an unbound variable for bash 3.2, which is the
# installed version on macOS. likewise, "${var[@]}" is an unbound
# error if var is an empty array.

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
. "${ci_dir}"/shared.sh

# zig cc is very slow: only use a few targets.
TARGETS=(
    "aarch64-unknown-linux-gnu"
    "aarch64-unknown-linux-musl"
    # disabled, see https://github.com/cross-rs/cross/issues/1425
    #"i586-unknown-linux-gnu"
    #"i586-unknown-linux-musl"
)

# on CI, it sets `CROSS_TARGET_ZIG_IMAGE` rather than `CROSS_BUILD_ZIG_IMAGE`
if [[ -n "${CROSS_TARGET_ZIG_IMAGE}" ]]; then
    export CROSS_BUILD_ZIG_IMAGE="${CROSS_TARGET_ZIG_IMAGE}"
    unset CROSS_TARGET_ZIG_IMAGE
fi

main() {
    export CROSS_BUILD_ZIG=1

    local td=
    local target=

    retry cargo fetch
    cargo build
    CROSS=$(binary_path cross "${PROJECT_HOME}" debug)
    export CROSS

    td="$(mktemp -d)"
    git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"
    pushd "${td}"

    for target in "${TARGETS[@]}"; do
        CROSS_CONTAINER_ENGINE="${CROSS_ENGINE}" "${CROSS}" build --target "${target}" --verbose
        # note: ensure #724 doesn't replicate during CI.
        # https://github.com/cross-rs/cross/issues/724
        cargo clean
    done

    popd
    rm -rf "${td}"
}

main "${@}"
