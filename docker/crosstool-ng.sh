#!/bin/bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local config="${1}"
    local nproc="${2}"
    local ctng_version=1.24.0
    local ctng_url="https://ci-mirrors.rust-lang.org/rustc/crosstool-ng-${ctng_version}.tar.gz"
    local username=crosstool
    local crosstooldir=/opt/crosstool
    local buildir
    local srcdir="/home/${username}/src"
    local dstdir="/x-tools"
    local sleep=15s
    local timeout=5m

    install_packages autoconf \
        bison \
        curl \
        flex \
        gawk \
        help2man \
        libncurses-dev \
        libtool-bin \
        patch \
        python3 \
        python3-dev \
        python3-pip \
        texinfo \
        wget \
        unzip \
        xz-utils

    # configure and install crosstool-ng
    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "${ctng_url}" | tar xzf -
    pushd "crosstool-ng-crosstool-ng-${ctng_version}"
    ./bootstrap
    ./configure --prefix="${crosstooldir}"
    make -j"${nproc}"
    make install

    popd
    popd

    # configure and install our toolchain
    buildir="$(mktemp -d)"

    # copy our config files, and make sure the l
    # crosstool-ng can't be run as root, so we do this instead.
    adduser --disabled-password --gecos "" "${username}"
    chown -R "${username}":"${username}" "${buildir}"
    pushd "${buildir}"
    cp /"${config}" .config
    chown "${username}":"${username}" .config

    # the download steps can stall indefinitely, so we want to set a timeout to
    # ensure it always completes. we therefore attempt to  download until
    # this step completes or fails. the built toolchain installs to `/x-tools`.
    mkdir -p "${dstdir}"
    chown -R "${username}":"${username}" "${dstdir}"
    local step=companion_tools_for_build
    su "${username}" -c "mkdir -p ${srcdir}"
    download() {
        # timeout is a command, not a built-in, so it won't
        # work with any bash functions: must call a command.
        timeout "${timeout}" \
            su "${username}" -c \
            "STOP=${step} CT_DEBUG_CT_SAVE_STEPS=1 ${crosstooldir}/bin/ct-ng build.${nproc} &> /dev/null"
    }

    while download; [ $? -eq 124 ]; do
        # Indicates a timeout, repeat the command.
        sleep "${sleep}"
    done
    su "${username}" -c "CT_DEBUG_CT_SAVE_STEPS=1 ${crosstooldir}/bin/ct-ng build.${nproc} &> /dev/null"

    popd

    purge_packages

    rm -rf "${srcdir}"
    rm -rf "${buildir}"
    rm -rf "${crosstooldir}"
    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
