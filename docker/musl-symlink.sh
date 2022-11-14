#!/usr/bin/env bash
# Create necessary symlinks for musl images to run
# dynamically-linked binaries.
# Just to be careful, we need this in a few locations,
# relative to the musl sysroot.
#   /lib/ld-musl-armhf.so
#   /lib/ld-musl-armhf.so.1
#   /usr/lib/ld.so
#   /usr/lib/ld.so.1
#   /usr/lib/libc.so
#   /usr/lib/libc.so.1

set -x
set -euo pipefail

main() {
    local sysroot="${1}"
    local arch="${2}"
    local src
    local dst
    local dsts

    # ignore any failures here
    local ld_arch="${arch//_/-}"
    mkdir -p "$sysroot/usr/lib"
    src="$sysroot/lib/libc.so"
    dsts=(
        "/lib/ld-musl-${arch}.so"
        "/lib/ld-musl-${arch}.so.1"
        "$sysroot/lib/ld-musl-${arch}.so"
        "$sysroot/lib/ld-musl-${arch}.so.1"
        "$sysroot/usr/lib/ld.so"
        "$sysroot/usr/lib/ld.so.1"
        "$sysroot/usr/lib/libc.so"
        "$sysroot/usr/lib/libc.so.1"
        # this specifically is a workaround for ARM64, which
        # for some reason links to `ld-linux-aarch64.so`, but
        # it is a valid musl binary. trying to use `libc6-dev:arm64`
        # shows it has an invalid ELF header.
        "$sysroot/lib/ld-linux-${ld_arch}.so"
        "$sysroot/lib/ld-linux-${ld_arch}.so.1"
    )
    for dst in "${dsts[@]}"; do
        # force a link if the dst does not exist or is broken
        if [[ -L "${dst}" ]] && [[ ! -a "${dst}" ]]; then
            ln -sf "${src}" "${dst}"
        elif [[ ! -f "${dst}" ]]; then
            ln -s "${src}" "${dst}"
        fi
    done

    # ensure we statically link libstdc++, so avoid segfaults with c++
    # https://github.com/cross-rs/cross/issues/902
    rm "${sysroot}"/lib/libstdc++.so* || true

    # now, we create a linker script that adds all the required dependencies
    # because we link to a static libstdc++ to avoid runtime issues and
    # with the shared libstdc++, we can have missing symbols that are reference
    # in libstdc++, such as those from libc like `setlocale` and `__cxa_atexit`,
    # as well as those from libgcc, like `__extendsftf2`. all musl targets
    # can require symbols from libc, however, only the following are known
    # to require symbols from libgcc:
    #   - aarch64-unknown-linux-musl
    #   - mips64-unknown-linux-muslabi64
    #   - mips64el-unknown-linux-muslabi64
    echo '/* cross-rs linker script
 * this allows us to statically link libstdc++ to avoid segfaults
 * https://github.com/cross-rs/cross/issues/902
 */
GROUP ( libstdc++.a AS_NEEDED( -lgcc -lc -lm ) )
' > "${sysroot}"/lib/libstdc++.so.6.0.27
    ln -s libstdc++.so.6.0.27 "${sysroot}"/lib/libstdc++.so.6
    ln -s libstdc++.so.6.0.27 "${sysroot}"/lib/libstdc++.so

    echo "${sysroot}/lib" >> "/etc/ld-musl-${arch}.path"

    rm -rf "${0}"
}

main "${@}"
