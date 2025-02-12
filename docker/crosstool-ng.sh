#!/bin/bash

set -x
set -eo pipefail

# shellcheck disable=SC1091
. lib.sh

silence_stdout() {
    if [[ "${VERBOSE}" == "1" ]]; then
        "${@}"
    else
        "${@}" >/dev/null
    fi
}

main() {
    local config="${1}"
    local nproc="${2}"
    local ctng_version=${3:-crosstool-ng-1.27.0}
    local ctng_url="https://github.com/crosstool-ng/crosstool-ng"
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
        rsync \
        texinfo \
        wget \
        unzip \
        xz-utils

    # configure and install crosstool-ng
    local td
    td="$(mktemp -d)"

    pushd "${td}"

    mkdir "crosstool-ng-${ctng_version}"
    pushd "crosstool-ng-${ctng_version}"
    git init
    git fetch --depth=1 "${ctng_url}" "${ctng_version}"
    git reset --hard FETCH_HEAD
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
    su "${username}" -c "${crosstooldir}/bin/ct-ng olddefconfig"

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
            "STOP=${step} CT_DEBUG_CT_SAVE_STEPS=1 ${crosstooldir}/bin/ct-ng build.${nproc}"
    }

    while silence_stdout download; [ $? -eq 124 ]; do
        # Indicates a timeout, repeat the command.
        sleep "${sleep}"
    done
    silence_stdout su "${username}" \
        -c "CT_DEBUG_CT_SAVE_STEPS=1 ${crosstooldir}/bin/ct-ng build.${nproc}"

    popd

    purge_packages

    rm -rf "${srcdir}"
    rm -rf "${buildir}"
    rm -rf "${crosstooldir}"
    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
