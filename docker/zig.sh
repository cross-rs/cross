#!/usr/bin/env bash

set -x
set -eo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local platform="${1}"
    install_packages ca-certificates curl xz-utils

    install_zig "${platform}"
    install_zigbuild "${platform}"

    purge_packages
    rm "${0}"
}

install_zig() {
    local platform="${1}"
    local version="0.11.0"
    local dst="/opt/zig"
    local arch=
    local os=
    local triple=

    case "${platform}" in
        'linux/386')
            arch="i386"
            os="linux"
            ;;
        'linux/amd64')
            arch="x86_64"
            os="linux"
            ;;
        'linux/arm64')
            arch="aarch64"
            os="linux"
            ;;
        'linux/riscv64')
            arch="riscv64"
            os="linux"
            ;;
        'linux/ppc64le')
            triple="powerpc64le-linux-gnu"
            ;;
        'linux/s390x')
            triple="s390x-linux-gnu"
            ;;
        'darwin/amd64')
            arch="x86_64"
            os="macos"
            ;;
        'darwin/arm64')
            arch="aarch64"
            os="macos"
            ;;
        # NOTE: explicitly don't support linux/arm/v6
        *)
            echo "Unsupported target platform '${platform}'" 1>&2
            exit 1
            ;;
    esac

    if [[ -n "${arch}" ]]; then
        install_zig_tarball "${arch}" "${os}" "${version}" "${dst}"
    else
        install_zig_source "${triple}" "${version}" "${dst}"
    fi
}

install_zig_tarball() {
    local arch="${1}"
    local os="${2}"
    local version="${3}"
    local dst="${4}"
    local filename="zig-${os}-${arch}-${version}.tar.xz"

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://ziglang.org/download/${version}/${filename}" -O
    mkdir -p "${dst}"
    tar --strip-components=1 -xJf "${filename}" --directory "${dst}"

    popd

    rm -rf "${td}"
}

install_zig_source() {
    local triple="${1}"
    local version="${2}"
    local dst="${3}"
    local filename="zig-bootstrap-${version}.tar.xz"

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://ziglang.org/download/${version}/${filename}" -O
    mkdir zig
    tar --strip-components=1 -xJf "${filename}" --directory zig

    pushd zig
    install_packages python3 make g++
    ./build -j5 "${triple}" native
    mv "out/zig-${triple}-native" /opt/zig

    popd
    popd

    rm -rf "${td}"
}

install_zigbuild() {
    local platform="${1}"
    local version="0.17.5"
    local dst="/usr/local"
    local triple=

    # we don't know if `linux/arm/v7` is hard-float,
    # and we don't know the the zigbuild `apple-darwin`
    # target doesn't manually specify the architecture.
    case "${platform}" in
        'linux/386')
            triple="i686-unknown-linux-musl"
            ;;
        'linux/amd64')
            triple="x86_64-unknown-linux-musl"
            ;;
        'linux/arm64')
            triple="aarch64-unknown-linux-musl"
            ;;
        *)
            ;;
    esac

    if [[ -n "${triple}" ]]; then
        install_zigbuild_tarball "${triple}" "${version}" "${dst}"
    else
        install_zigbuild_source "${version}" "${dst}"
    fi
}

install_zigbuild_tarball() {
    local triple="${1}"
    local version="${2}"
    local dst="${3}"
    local repo="https://github.com/messense/cargo-zigbuild"
    local filename="cargo-zigbuild-v${version}.${triple}.tar.gz"

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "${repo}/releases/download/v${version}/${filename}" -O
    mkdir -p "${dst}/bin"
    tar -xzf "${filename}" --directory "${dst}/bin"

    popd

    rm -rf "${td}"
}

install_zigbuild_source() {
    local version="${1}"
    local dst="${2}"

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    export RUSTUP_HOME="${td}/rustup"
    export CARGO_HOME="${td}/cargo"

    curl --retry 3 -sSfL https://sh.rustup.rs -o rustup-init.sh
    sh rustup-init.sh -y --no-modify-path --profile minimal

    PATH="${CARGO_HOME}/bin:${PATH}" \
        cargo install cargo-zigbuild \
        --version "${version}" \
        --root "${dst}" \
        --locked

    popd

    rm -rf "${td}"
}

main "${@}"
