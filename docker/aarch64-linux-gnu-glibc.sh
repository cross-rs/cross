#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

unpack_rpm() {
    local package="${1}"
    curl --retry 3 "http://mirror.centos.org/altarch/7/os/aarch64/Packages/${package}" -O
    rpm2cpio "${package}" | cpio -idmv
}

symlink_gcc_lib() {
    local prefix="${1}"
    shift
    local srcfile="${1}"
    shift
    local dstdir="/usr/lib/gcc/aarch64-linux-gnu"

    ln -s "${prefix}/lib/${srcfile}" "${dstdir}/4.8.2/${srcfile}"
    ln -s "${prefix}/lib/${srcfile}" "${dstdir}/4.8.5/${srcfile}"

    local dstfile
    for dstfile in "${@}"; do
        ln -s "${prefix}/lib/${srcfile}" "${dstdir}/4.8.2/${dstfile}"
        ln -s "${prefix}/lib/${srcfile}" "${dstdir}/4.8.5/${dstfile}"
    done
}

cp_gcc_archive() {
    local name="${1}"
    local srcdir="usr/lib/gcc/aarch64-redhat-linux/"
    local dstdir="/usr/lib/gcc/aarch64-linux-gnu/"
    cp "${srcdir}/4.8.2/${name}" "${dstdir}/4.8.2/${name}"
    cp "${srcdir}/4.8.5/${name}" "${dstdir}/4.8.5/${name}"
}

main() {
    set_centos_ulimit
    yum install -y epel-release
    yum install -y gcc-aarch64-linux-gnu gcc-c++-aarch64-linux-gnu gfortran-c++-aarch64-linux-gnu binutils-aarch64-linux-gnu binutils gcc-c++ glibc-devel
    yum clean all

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    local target=aarch64-linux-gnu
    local prefix="/usr/${target}"
    local kernel_v4="4.18.20"

    curl --retry 3 "https://mirrors.edge.kernel.org/pub/linux/kernel/v4.x/linux-${kernel_v4}.tar.xz" -O
    tar -xvf "linux-${kernel_v4}.tar.xz"
    pushd "linux-${kernel_v4}"
    make ARCH=arm64 INSTALL_HDR_PATH="${prefix}" headers_install
    popd

    curl --retry 3 http://ftp.gnu.org/gnu/glibc/glibc-2.17.tar.xz -O
    tar -xvf glibc-2.17.tar.xz
    mkdir build
    pushd build
    CC=/usr/bin/aarch64-linux-gnu-gcc \
        CXX=/usr/bin/aarch64-linux-gnu-g++ \
        LD=/usr/bin/aarch64-linux-gnu-ld \
        AR=/usr/bin/aarch64-linux-gnu-ar \
        RANLIB=/usr/bin/aarch64-linux-gnu-ranlib \
        ../glibc-2.17/configure \
        --prefix="${prefix}" \
        --build="${MACHTYPE}" \
        --host="${target}" \
        --target="${target}" \
        --with-arch=aarch64 \
        --with-headers="${prefix}/include" \
        --libdir="${prefix}/lib" \
        --libexecdir="${prefix}/lib"

    make -j && make install
    popd

    mkdir -p "${prefix}"/{include,lib}
    mkdir -p "/usr/lib/gcc/aarch64-linux-gnu"/{4.8.2,4.8.5}

    mkdir libgcc
    pushd libgcc
    unpack_rpm "libgcc-4.8.5-44.el7.aarch64.rpm"
    mv lib64/* "${prefix}/lib"
    # C++ support needs `libgcc.so`, even though it warns about `libgcc_s.so`
    symlink_gcc_lib "${prefix}" "libgcc_s.so.1" "libgcc_s.so" "libgcc.so"
    popd

    mkdir libstdcpp
    pushd libstdcpp
    unpack_rpm "libstdc++-4.8.5-44.el7.aarch64.rpm"
    unpack_rpm "libstdc++-devel-4.8.5-44.el7.aarch64.rpm"
    unpack_rpm "libstdc++-static-4.8.5-44.el7.aarch64.rpm"
    mv usr/include/* "${prefix}/include"
    mv usr/lib64/* "${prefix}/lib"
    symlink_gcc_lib "${prefix}" "libstdc++.so.6" "libstdc++.so"
    cp_gcc_archive "libstdc++.a"
    cp_gcc_archive "libsupc++.a"
    popd

    local cpp_include=/usr/aarch64-linux-gnu/include/c++
    local cpp_482="${cpp_include}/4.8.2"
    local cpp_485="${cpp_include}/4.8.5"
    local redhat_482="${cpp_482}/aarch64-redhat-linux"
    local redhat_485="${cpp_485}/aarch64-redhat-linux"
    mv "${redhat_482}/bits"/* "${cpp_482}/bits"
    mv "${redhat_482}/ext"/* "${cpp_482}/ext"
    # these are currently empty, but might contain content later
    mv "${redhat_485}/bits"/* "${cpp_485}/bits" || true
    mv "${redhat_485}/ext"/* "${cpp_485}/ext" || true

    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
