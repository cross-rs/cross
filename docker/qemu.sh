#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local version=5.0.0

    # Qemu versions 3.1.0 and above break 32-bit float conversions
    # on powerpc, powerpc64, and powerpc64le. Last known working version
    # is 3.0.1.
    # Upstream Issue:
    #   https://bugs.launchpad.net/qemu/+bug/1821444
    if [[ "${1}" == ppc* ]]; then
        version=3.0.1
    fi

    local arch="${1}" \
          softmmu="${2:-}"

    local dependencies=(
        autoconf
        automake
        bison
        bzip2
        curl
        flex
        g++
        libglib2.0-dev
        libtool
        make
        patch
        pkg-config
        python3
        zlib1g-dev
        libcap-dev
        libcap-ng-dev
        libattr1-dev
        libpixman-1-dev
        virtinst
        xz-utils
    )

    apt-get update
    local purge_list=()
    for dep in "${dependencies[@]}"; do
        if ! dpkg -L "${dep}"; then
            apt-get install --assume-yes --no-install-recommends "${dep}"
            purge_list+=( "${dep}" )
        fi
    done

    local td
    td="$(mktemp -d)"

    pushd "${td}"

    curl -L "https://download.qemu.org/qemu-${version}.tar.xz" | \
        tar --strip-components=1 -xJ

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
        --enable-user \
        --static \
        ${virtfs} \
        --target-list="${targets}"
    make "-j$(nproc)"
    make install

    # HACK the binfmt_misc interpreter we'll use expects the QEMU binary to be
    # in /usr/bin. Create an appropriate symlink
    ln -s "/usr/local/bin/qemu-${arch}" "/usr/bin/qemu-${arch}-static"

    if (( ${#purge_list[@]} )); then
      apt-get purge --assume-yes --auto-remove "${purge_list[@]}"
    fi

    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
