#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

build_static_libattr() {
    local version=2.4.46

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    yum install -y gettext

    curl --retry 3 -sSfL "https://download.savannah.nongnu.org/releases/attr/attr-${version}.src.tar.gz" -O
    tar --strip-components=1 -xzf "attr-${version}.src.tar.gz"
    ./configure
    make "-j$(nproc)"
    install -m 644 ./libattr/.libs/libattr.a /usr/lib64/

    yum remove -y gettext

    popd

    rm -rf "${td}"
}

build_static_libcap() {
    local version=2.22

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://www.kernel.org/pub/linux/libs/security/linux-privs/libcap2/libcap-${version}.tar.xz" -O
    tar --strip-components=1 -xJf "libcap-${version}.tar.xz"
    make "-j$(nproc)"
    install -m 644 libcap/libcap.a /usr/lib64/

    popd

    rm -rf "${td}"
}

build_static_pixman() {
    local version=0.34.0

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://www.cairographics.org/releases/pixman-${version}.tar.gz" -O
    tar --strip-components=1 -xzf "pixman-${version}.tar.gz"
    ./configure
    make "-j$(nproc)"
    install -m 644 ./pixman/.libs/libpixman-1.a /usr/lib64/

    popd

    rm -rf "${td}"
}

main() {
    local version=5.1.0

    local arch="${1}" \
          softmmu="${2:-}"

    install_packages \
        autoconf \
        automake \
        bison \
        bzip2 \
        curl \
        flex \
        libtool \
        make \
        patch \
        python3 \

    if_centos install_packages \
        gcc-c++ \
        pkgconfig \
        xz \
        glib2-devel \
        glib2-static \
        glibc-static \
        libattr-devel \
        libcap-ng-devel \
        libfdt-devel \
        pcre-static \
        pixman-devel \
        zlib-devel \
        zlib-static

    # these are not packaged as static libraries in centos; build them manually
    if_centos build_static_libattr
    if_centos build_static_libcap
    if_centos build_static_pixman

    if_ubuntu install_packages \
        g++ \
        pkg-config \
        xz-utils \
        libattr1-dev \
        libcap-ng-dev \
        libffi-dev \
        libglib2.0-dev \
        libpixman-1-dev \
        libselinux1-dev \
        zlib1g-dev

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://download.qemu.org/qemu-${version}.tar.xz" -O
    tar --strip-components=1 -xJf "qemu-${version}.tar.xz"

    local targets="${arch}-linux-user"
    local virtfs=""
    case "${softmmu}" in
        softmmu)
            if [ "${arch}" = "ppc64le" ]; then
                targets="${targets},ppc64-softmmu"
            else
                targets="${targets},${arch}-softmmu"
            fi
            virtfs="--enable-virtfs"
            ;;
        "")
            true
            ;;
        *)
            echo "Invalid softmmu option: ${softmmu}"
            exit 1
            ;;
    esac

    ./configure \
        --disable-kvm \
        --disable-vnc \
        --disable-guest-agent \
        --enable-linux-user \
        --static \
        ${virtfs} \
        --target-list="${targets}"
    make "-j$(nproc)"
    make install

    # HACK the binfmt_misc interpreter we'll use expects the QEMU binary to be
    # in /usr/bin. Create an appropriate symlink
    ln -s "/usr/local/bin/qemu-${arch}" "/usr/bin/qemu-${arch}-static"

    purge_packages

    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
