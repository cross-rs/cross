#!/usr/bin/env bash
# shellcheck disable=SC1004,SC1091,SC1090

# test to see that running docker-in-docker works

set -x
set -eo pipefail

if [[ -z "${TARGET}" ]]; then
    export TARGET="aarch64-unknown-linux-gnu"
fi
# ^^subst is not supported on macOS bash (bash <4)
# shellcheck disable=SC2155
export TARGET_UPPER=$(echo "$TARGET" | awk '{print toupper($0)}')

if [[ "${IMAGE}" ]]; then
    # shellcheck disable=SC2140
    export "CROSS_TARGET_${TARGET_UPPER//-/_}_IMAGE"="${IMAGE}"
fi

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
. "${ci_dir}"/shared.sh

main() {
    docker run --platform linux/amd64 -v "${PROJECT_HOME}":"${PROJECT_HOME}" -w "${PROJECT_HOME}" \
        --rm -e TARGET -e TARGET_UPPER -e RUSTFLAGS -e RUST_TEST_THREADS \
        -e LLVM_PROFILE_FILE -e CARGO_INCREMENTAL \
        -e "CROSS_TARGET_${TARGET_UPPER//-/_}_IMAGE" \
        -v /var/run/docker.sock:/var/run/docker.sock \
        docker:20.10-dind sh -c '
#!/usr/bin/env sh
set -x
set -euo pipefail

apk add curl
curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "${HOME}/.cargo/env"

# building on release is slow
apk add libgcc gcc musl-dev
cargo test --workspace
cargo install --path . --force --debug

export CROSS_CONTAINER_IN_CONTAINER=1

apk add git
td="$(mktemp -d)"
git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"
cd "${td}"
cross run --target "${TARGET}" --verbose

td="$(mktemp -d)"
git clone --depth 1 https://github.com/cross-rs/test-workspace "${td}"
cd "${td}"
cross build --target "${TARGET}" --workspace \
    --manifest-path="./workspace/Cargo.toml" --verbose
eval CROSS_TARGET_${TARGET_UPPER//-/_}_PRE_BUILD="exit" cross build --target "${TARGET}" --workspace \
    --manifest-path="./workspace/Cargo.toml" --verbose
cd workspace
cross build --target "${TARGET}" --workspace --verbose
eval CROSS_TARGET_${TARGET_UPPER//-/_}_PRE_BUILD="exit" cross build --target "${TARGET}" --workspace --verbose
cd binary
cross run --target "${TARGET}" --verbose
eval CROSS_TARGET_${TARGET_UPPER//-/_}_PRE_BUILD="exit" cross run --target "${TARGET}" --verbose
'
}

main
