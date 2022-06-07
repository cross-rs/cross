#!/usr/bin/env bash
#
# this script can be customized with the following env vars:
#       CROSS_IMAGE: defaults to `ghcr.io/cross-rs`
#       CROSS_CONTAINER_ENGINE: defaults to `docker` or `podman`
#       CROSS_IMAGE_VERSION: defaults to `main`.
#
# if no arguments are provided, this script will process all
# images. you can extract target info for specific targets
# by providing them as arguments after the script, for example,
# `./extract_target_info.sh i686-linux-android`.
#
# the output will be similar to the following:
#       | `aarch64-unknown-linux-musl`         | 1.2.0  | 9.2.0   | ✓   | 5.1.0 |
#       | `i686-linux-android`                 | 9.0.8  | 9.0.8   | ✓   | 5.1.0 |
#       | `i686-unknown-linux-musl`            | 1.2.0  | 9.2.0   | ✓   | N/A   |
#       ...
#
# in short, it recreates the table except for the test section in README.md.

# shellcheck disable=SC2207

set -eo pipefail

scriptdir=$(dirname "${BASH_SOURCE[0]}")
scriptdir=$(realpath "${scriptdir}")
project_dir=$(dirname "${scriptdir}")

if [[ -z "$CROSS_IMAGE" ]]; then
    CROSS_IMAGE="ghcr.io/cross-rs"
fi
if [[ -z "$CROSS_CONTAINER_ENGINE" ]]; then
    if command -v "docker" &>/dev/null; then
        CROSS_CONTAINER_ENGINE="docker"
    elif command -v "podman" &>/dev/null; then
        CROSS_CONTAINER_ENGINE="podman"
    else
        echo "Unable to find suitable container engine." 1>&2
        exit 1
    fi
fi
if [[ -z "$CROSS_IMAGE_VERSION" ]]; then
    CROSS_IMAGE_VERSION="main"
fi

pull() {
    "${CROSS_CONTAINER_ENGINE}" pull "${1}"
}

run() {
    TARGET="${1}" "${CROSS_CONTAINER_ENGINE}" run --rm -e TARGET \
        -v "${scriptdir}:/ci:ro" "${2}" \
        bash -c "/ci/extract_image_info.sh"
}

# parse our CI list, so updating our CI automatically updates our target list.
if [[ $# -eq "0" ]]; then
    ci="${project_dir}"/.github/workflows/ci.yml
    matrix=$(yq '."jobs"."generate-matrix"."steps".0."env"."matrix"' "${ci}")
    targets=($(yq '.[]."target"' <<< "${matrix}"))
else
    targets=("${@}")
fi
for target in "${targets[@]}"; do
    # can't do MSVC, Darwin, or iOS images.
    case "${target}" in
        *-msvc | *-darwin | *-apple-ios)
            continue
            ;;
    esac

    image="${CROSS_IMAGE}"/"${target}":"${CROSS_IMAGE_VERSION}"
    if [[ -z "$DEBUG" ]]; then
        pull "${image}" >/dev/null 2>&1
        run "${target}" "${image}" 2>/dev/null
    else
        pull "${image}"
        run "${target}" "${image}"
    fi
done
