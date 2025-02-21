#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

build_static_libffi () {
    local version=3.0.13

    local td
    td="$(mktemp -d)"

    pushd "${td}"


    curl --retry 3 -sSfL "https://github.com/libffi/libffi/archive/refs/tags/v${version}.tar.gz" -O -L
    tar --strip-components=1 -xzf "v${version}.tar.gz"
    ./configure --prefix="$td"/lib --disable-builddir --disable-shared --enable-static
    make "-j$(nproc)"
    install -m 644 ./.libs/libffi.a /usr/local/lib/

    popd

    rm -rf "${td}"
}

build_static_libmount () {
    local version_spec=2.23.2
    local version=2.23

    if_ubuntu_ge 22.04 version_spec=2.37.2
    if_ubuntu_ge 22.04 version=2.37

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://kernel.org/pub/linux/utils/util-linux/v${version}/util-linux-${version_spec}.tar.xz" -O -L
    tar --strip-components=1 -xJf "util-linux-${version_spec}.tar.xz"
    ./configure --disable-shared --enable-static --without-ncurses
    make "-j$(nproc)" mount blkid
    install -m 644 ./.libs/*.a /usr/local/lib/

    popd

    rm -rf "${td}"
}


build_static_libattr() {
    local version=2.4.46

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    set_centos_ulimit
    yum install -y gettext

    curl --retry 3 -sSfL "https://download.savannah.nongnu.org/releases/attr/attr-${version}.src.tar.gz" -O
    tar --strip-components=1 -xzf "attr-${version}.src.tar.gz"
    cp /usr/share/automake*/config.* .

    ./configure
    make "-j$(nproc)"
    install -m 644 ./libattr/.libs/libattr.a /usr/local/lib/

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
    install -m 644 libcap/libcap.a /usr/local/lib/

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
    install -m 644 ./pixman/.libs/libpixman-1.a /usr/local/lib/

    popd

    rm -rf "${td}"
}

build_static_slirp() {
    local version=4.1.0

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl --retry 3 -sSfL "https://gitlab.freedesktop.org/slirp/libslirp//-/archive/v${version}/libslirp-v${version}.tar.gz" -O
    tar -xzf "libslirp-v${version}.tar.gz"
    meson setup -Ddefault_library=static libslirp-v${version} build
    ninja -C build
    install -m 644 ./build/libslirp.a /usr/local/lib/

    popd

    rm -rf "${td}"
}

main() {
    local version=5.1.0

    if_centos version=4.2.1

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
        libcap-devel \
        libfdt-devel \
        pcre-static \
        pixman-devel \
        libselinux-devel \
        libselinux-static \
        libffi \
        libuuid-devel \
        libblkid-devel \
        libmount-devel \
        zlib-devel \
        zlib-static

    if_centos 'curl --retry 3 -sSfL "https://git.savannah.gnu.org/gitweb/?p=config.git;a=blob_plain;f=config.guess;hb=HEAD" -o /usr/share/automake*/config.guess'
    if_centos 'curl --retry 3 -sSfL "https://git.savannah.gnu.org/gitweb/?p=config.git;a=blob_plain;f=config.sub;hb=HEAD" -o /usr/share/automake*/config.sub'

    # these are not packaged as static libraries in centos; build them manually
    if_centos build_static_libffi
    if_centos build_static_libmount
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

    # ubuntu no longer provides statically linked libmount
    if_ubuntu_ge 22.04 build_static_libmount

    # if we have python3.6+, we can install qemu 7.0.0, which needs ninja-build
    # ubuntu 16.04 only provides python3.5, so remove when we have a newer qemu.
    is_ge_python36=$(python3 -c "import sys; print(int(sys.version_info >= (3, 6)))")
    if [[ "${is_ge_python36}" == "1" ]]; then
        if_ubuntu version=7.0.0
        if_ubuntu install_packages ninja-build
    fi

    # if we have python3.8+, we can install qemu 8.2.2, which needs ninja-build,
    # meson, python3-pip and libslirp-dev.
    # ubuntu 16.04 only provides python3.5, so remove when we have a newer qemu.
    is_ge_python38=$(python3 -c "import sys; print(int(sys.version_info >= (3, 8)))")
    if [[ "${is_ge_python38}" == "1" ]]; then
        if_ubuntu version=8.2.2
        if_ubuntu install_packages ninja-build meson python3-pip libslirp-dev
        if_ubuntu build_static_slirp
    fi

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
