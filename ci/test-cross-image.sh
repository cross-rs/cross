#!/usr/bin/env bash
# shellcheck disable=SC2086

set -x
set -eo pipefail

if [[ -z "${TARGET}" ]]; then
    export TARGET="aarch64-unknown-linux-gnu"
fi

if [[ "${IMAGE}" ]]; then
    # shellcheck disable=SC2140
    export "CROSS_TARGET_${TARGET//-/_}_IMAGE"="${IMAGE}"
fi

if [[ -z "${CROSS_TARGET_CROSS_IMAGE}" ]]; then
    CROSS_TARGET_CROSS_IMAGE="ghcr.io/cross-rs/cross:main"
fi


main() {

    docker run --rm -e TARGET -e "CROSS_TARGET_${TARGET//-/_}_IMAGE" \
        -v /var/run/docker.sock:/var/run/docker.sock \
        "${CROSS_TARGET_CROSS_IMAGE}" sh -c '
#!/usr/bin/env sh
td="$(mktemp -d)"
git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"
cd "${td}"
cross run --target "${TARGET}"
'
}

main "${@}"
