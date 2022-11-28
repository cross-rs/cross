#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. freebsd-common.sh
# shellcheck disable=SC1091
. lib.sh

max_freebsd() {
    local best=
    local minor=0
    local version=
    local release_major=
    local release_minor=
    for release in "${@}"; do
        version=$(echo "${release}" | cut -d '-' -f 1)
        release_major=$(echo "${version}"| cut -d '.' -f 1)
        release_minor=$(echo "${version}"| cut -d '.' -f 2)
        if [ "${release_major}" == "${BSD_MAJOR}" ] && [ "${release_minor}" -gt "${minor}" ]; then
            best="${release}"
            minor="${release_minor}"
        fi
    done
    if [[ -z "$best" ]]; then
        echo -e "\e[31merror:\e[0m could not find best release for FreeBSD ${BSD_MAJOR}." 1>&2
        exit 1
    fi
    echo "${best}"
}

latest_freebsd() {
    local mirror="${1}"
    local response=
    local line=
    local lines=
    local releases=
    local max_release=

    response=$(curl --silent --list-only --location "${mirror}/${BSD_ARCH}/" | grep RELEASE)
    if [[ "${response}" != *RELEASE* ]]; then
        echo -e "\e[31merror:\e[0m could not find a candidate release for FreeBSD ${BSD_MAJOR}." 1>&2
        exit 1
    fi
    readarray -t lines <<< "${response}"

    # shellcheck disable=SC2016
    local regex='/<a.*?>\s*(\d+\.\d+-RELEASE)\s*\/?\s*<\/a>/; print $1'
    # not all lines will match: some return `*-RELEASE/` as a line
    if [[ "${response}" == *"<a"* ]]; then
        # have HTML output, need to extract it via a regex
        releases=()
        for line in "${lines[@]}"; do
            if [[ "${line}" == *"<a"* ]]; then
                # because of the pattern we're extracting, this can't split
                # shellcheck disable=SC2207
                releases+=($(echo "${line}" | perl -nle "${regex}"))
            fi
        done
    else
        releases=("${lines[@]}")
    fi

    max_release=$(max_freebsd "${releases[@]}")
    echo "${max_release//-RELEASE/}"
}

freebsd_mirror() {
    local home=
    local code=

    set +e
    for home in "${BSD_HOME[@]}"; do
        # we need a timeout in case the server is down to avoid
        # infinitely hanging. timeout error code is always 124
        # these mirrors can be quite slow, so have a long timeout
        timeout 20s curl --silent --list-only --location "${home}/${BSD_ARCH}/" >/dev/null
        code=$?
        if [[ "${code}" == 0 ]]; then
            echo "${home}"
            return 0
        elif [[ "${code}" != 124 ]]; then
            echo -e "\e[1;33mwarning:\e[0m mirror ${home} does not seem to work." 1>&2
        fi
    done
    set -e

    echo -e "\e[31merror:\e[0m could not find a working FreeBSD mirror." 1>&2
    exit 1
}

mirror=$(freebsd_mirror)
base_release=$(latest_freebsd "${mirror}")
bsd_base_url="${mirror}/${BSD_ARCH}/${base_release}-RELEASE"
if [[ "${bsd_base_url}" == "http"* ]]; then
    bsd_url="${bsd_base_url}"
else
    bsd_url="http://${bsd_base_url}"
fi

main() {
    local binutils=2.32 \
        gcc=6.4.0 \
        target="${ARCH}-unknown-freebsd${BSD_MAJOR}"

    install_packages ca-certificates \
        curl \
        g++ \
        make \
        wget \
        xz-utils

    local td
    td="$(mktemp -d)"
    pushd "${td}"

    mkdir "${td}"/{binutils,gcc}{,-build} "${td}/freebsd"

    download_binutils "${binutils}" "gz"
    tar -C "${td}/binutils" --strip-components=1 -xf "binutils-${binutils}.tar.gz"

    download_gcc "${gcc}" "gz"
    tar -C "${td}/gcc" --strip-components=1 -xf "gcc-${gcc}.tar.gz"

    cd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    cd ..

    curl --retry 3 -sSfL "${bsd_url}/base.txz" -O
    tar -C "${td}/freebsd" -xJf base.txz ./usr/include ./usr/lib ./lib

    cd binutils-build
    ../binutils/configure \
        --target="${target}"
    make "-j$(nproc)"
    make install
    cd ..

    local destdir="/usr/local/${target}"
    cp -r "${td}/freebsd/usr/include" "${destdir}"
    cp -r "${td}/freebsd/lib/"* "${destdir}/lib"
    cp "${td}/freebsd/usr/lib/libc++.so.1" "${destdir}/lib"
    cp "${td}/freebsd/usr/lib/libc++.a" "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/lib{c,util,m,ssp_nonshared}.a "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/lib{rt,execinfo,procstat}.so.1 "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/{crt1,Scrt1,crti,crtn}.o "${destdir}/lib"
    cp "${td}/freebsd/usr/lib"/libkvm.a "${destdir}/lib"

    local lib=
    local base=
    local link=
    for lib in "${destdir}/lib/"*.so.*; do
        base=$(basename "${lib}")
        link="${base}"
        # not strictly necessary since this will always work, but good fallback
        while [[ "${link}" == *.so.* ]]; do
            link="${link%.*}"
        done

        # just extra insurance that we won't try to overwrite an existing file
        local dstlink="${destdir}/lib/${link}"
        if [[ -n "${link}" ]] && [[ "${link}" != "${base}" ]] && [[ ! -f "${dstlink}" ]]; then
            ln -s "${base}" "${dstlink}"
        fi
    done

    ln -s libthr.so.3 "${destdir}/lib/libpthread.so"

    cd gcc-build
    ../gcc/configure \
        --disable-libada \
        --disable-libcilkrt \
        --disable-libcilkrts \
        --disable-libgomp \
        --disable-libquadmath \
        --disable-libquadmath-support \
        --disable-libsanitizer \
        --disable-libssp \
        --disable-libvtv \
        --disable-lto \
        --disable-nls \
        --enable-languages=c,c++ \
        --target="${target}"
    make "-j$(nproc)"
    make install
    cd ..

    # clean up
    popd

    purge_packages

    # store the version info for the FreeBSD release
    bsd_revision=$(curl --retry 3 -sSfL "${bsd_url}/REVISION")
    echo "${base_release} (${bsd_revision})" > /opt/freebsd-version

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
