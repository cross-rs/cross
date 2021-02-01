#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local version=apple/stable/20200714

    install_packages curl ninja-build python3 llvm clang lld

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://github.com/apple/llvm-project/archive/${version}.tar.gz" -o apple-llvm.tar.gz
    tar --strip-components=1 -xaf apple-llvm.tar.gz

	mkdir build && cd build

    cmake -G Ninja ../llvm \
		-DCMAKE_BUILD_TYPE=Release \
		-DCMAKE_INSTALL_PREFIX=/usr/local \
		-DLLVM_TARGETS_TO_BUILD="AArch64" \
		-DLLVM_BUILD_TOOLS=On \
		-DLLVM_INCLUDE_TOOLS=On \
		-DLLVM_INSTALL_BINUTILS_SYMLINKS=On \
		-DLLVM_INSTALL_CCTOOLS_SYMLINKS=On \
		-DLLVM_BUILD_EXAMPLES=Off \
		-DLLVM_INCLUDE_EXAMPLES=Off \
		-DLLVM_BUILD_TESTS=Off \
		-DLLVM_INCLUDE_TESTS=Off \
		-DLLVM_BUILD_BENCHMARKS=Off \
		-DLLVM_INCLUDE_BENCHMARKS=Off \
		-DLLVM_ENABLE_PROJECTS="clang;lld" \
		-DLLVM_USE_LINKER=lld \
		-DCMAKE_C_COMPILER=clang \
		-DCMAKE_CXX_COMPILER=clang++
	
	cmake --build .
	cmake --install .

    purge_packages

    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
