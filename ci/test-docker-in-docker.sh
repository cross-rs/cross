#!/usr/bin/env bash
# shellcheck disable=SC1004

# test to see that running docker-in-docker works

set -x
set -eo pipefail

if [[ -z "${TARGET}" ]]; then
    export TARGET="aarch64-unknown-linux-gnu"
fi

if [[ "${IMAGE}" ]]; then
    # shellcheck disable=SC2140
    export "CROSS_TARGET_${TARGET//-/_}_IMAGE"="${IMAGE}"
fi

source=$(dirname "${BASH_SOURCE[0]}")
source=$(realpath "${source}")
home=$(dirname "${source}")

main() {
    docker run -v "${home}":"${home}" -w "${home}" \
        --rm -e TARGET -e RUSTFLAGS -e RUST_TEST_THREADS \
        -e LLVM_PROFILE_FILE -e CARGO_INCREMENTAL \
        -e "CROSS_TARGET_${TARGET//-/_}_IMAGE" \
        -v /var/run/docker.sock:/var/run/docker.sock \
        docker:18.09-dind sh -c '
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
cd workspace
cross build --target "${TARGET}" --workspace --verbose
cd binary
cross run --target "${TARGET}" --verbose
'
}

main
