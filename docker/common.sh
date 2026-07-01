#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

# Install ca-certificates before enabling HTTPS in sources
if grep -q -i ubuntu /etc/os-release; then
    install_packages ca-certificates
    sed -i 's|http://|https://|' /etc/apt/sources.list.d/ubuntu.sources
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
    bzip2 \
    adduser \
    g++ \
    gfortran \
    libc6-dev \
    libclang-dev \
    pkg-config
