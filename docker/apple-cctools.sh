#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local cctools_commit=30518813875aed656aa7f18b6d485feee25f8f87

    install_packages curl python3 clang
    # Don't use install_packages, we want to keep this.
    apt-get install --assume-yes --no-install-recommends libssl-dev

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://github.com/okanon/iPhoneOS.sdk/releases/download/v0.0.1/iPhoneOS13.2.sdk.tar.gz" -o iPhoneOS13.2.sdk.tar.gz
    curl --retry 3 -sSfL "https://github.com/tpoechtrager/cctools-port/archive/${cctools_commit}.tar.gz" -O
    tar --strip-components=1 -xaf "${cctools_commit}.tar.gz"

	cd usage_examples/ios_toolchain
	sed -i "s/arm-apple-darwin11/aarch64-apple-darwin/" build.sh
    ./build.sh "${td}/iPhoneOS13.2.sdk.tar.gz" arm64

	mkdir -p /usr/local/bin
	cp -af target/bin/* /usr/local/bin

    purge_packages


    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
