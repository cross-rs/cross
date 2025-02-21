#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

# For non-native architectures, look for packages on ports.ubuntu.com instead.
# This is important if you enable additional architectures so you can install libraries to cross-compile against.
# Look for 'dpkg --add-architecture' in the README for more details.
if grep -i ubuntu /etc/os-release >/dev/null; then
    NATIVE_ARCH=$(dpkg --print-architecture)

    if [ "$NATIVE_ARCH" = "amd64" ]; then
        sed 's/http:\/\/\(.*\).ubuntu.com\/ubuntu\//[arch-=amd64,i386] http:\/\/ports.ubuntu.com\/ubuntu-ports\//g' /etc/apt/sources.list > /etc/apt/sources.list.d/ports.list
        sed -i 's/http:\/\/\(.*\).ubuntu.com\/ubuntu\//[arch=amd64,i386] http:\/\/\1.archive.ubuntu.com\/ubuntu\//g' /etc/apt/sources.list
    else
        sed -i "s/http:\/\/\(.*\).ubuntu.com\/ubuntu\//[arch-=${NATIVE_ARCH}] http:\/\/ports.ubuntu.com\/ubuntu-ports\//g" /etc/apt/sources.list
        sed -i "s/http:\/\/\(.*\).ubuntu.com\/ubuntu\//[arch=${NATIVE_ARCH}] http:\/\/\1.archive.ubuntu.com\/ubuntu\//g" /etc/apt/sources.list
    fi
fi

install_packages \
    autoconf \
    automake \
    binutils \
    ca-certificates \
    curl \
    file \
    gcc \
    git \
    libtool \
    m4 \
    make

if_centos install_packages \
    clang-devel \
    gcc-c++ \
    gcc-gfortran \
    glibc-devel \
    pkgconfig

if_ubuntu install_packages \
    g++ \
    gfortran \
    libc6-dev \
    libclang-dev \
    pkg-config
