#!/usr/bin/env bash

host_architecture() {
    # there's numerous compatibility modes, so we want
    # to ensure that these are valid. we also want to
    # use dpkg if it's available since it gives hard-float
    # information on compatible architectures
    local host
    local arch
    if dpkg >/dev/null 2>&1; then
        host=$(dpkg --print-architecture)
        arch="${host}"
    else
        host=$(uname -m)
        arch="${host}"

        case "${arch}" in
            aarch64|armv8b|armv8l)
                arch=arm64
                ;;
            aarch64_be)
                arch=arm64be
                ;;
            arm*)
                arch=unknown
                ;;
            ppc)
                arch=powerpc
                ;;
            ppc64le)
                arch=ppc64el
                ;;
            s390)
                arch=s390x
                ;;
            i?86)
                arch=i386
                ;;
            x64|x86_64)
                arch=amd64
                ;;
            *)
                ;;
        esac
    fi

    echo "${arch}"
}

normalize_arch() {
    local arch="${1}"
    local debian

    debian="${arch}"
    case "${arch}" in
        aarch64)
            debian=arm64
            ;;
        x86_64)
            debian=amd64
            ;;
        arm)
            debian=armel
            ;;
        armv7)
            debian=arm
            ;;
        armv7hf)
            debian=armhf
            ;;
        i?86)
            debian=i386
            ;;
        powerpc64)
            debian=ppc64
            ;;
        powerpc64le)
            debian=ppc64el
            ;;
        riscv64*)
            debian=riscv64
            ;;
        *)
            ;;
    esac

    echo "${debian}"
}

is_native_binary() {
    # determines if the binary can run natively on the host
    local arch="${1}"
    local host
    local target
    host=$(host_architecture)
    target=$(normalize_arch "${arch}")

    # FIXME: this is not comprehensive. add more compatible architectures.
    case "${host}" in
        amd64)
            if [[ "${target}" == i386 ]] || [[ "${target}" == amd64 ]]; then
                return 0
            fi
            ;;
        *)
            if [[ "${host}" == "${target}" ]]; then
                return 0
            fi
            ;;
    esac

    return 1
}

qemu_arch() {
    # select qemu arch
    local arch="${1}"
    local qarch="${arch}"
    case "${arch}" in
        arm|armhf|armv7|armv7hf)
            qarch="arm"
            ;;
        i?86)
            qarch="i386"
            ;;
        powerpc)
            qarch="ppc"
            ;;
        powerpc64)
            qarch="ppc64"
            ;;
        powerpc64le)
            if [ "${CROSS_RUNNER}" = "qemu-user" ]; then
                qarch="ppc64le"
            else
                qarch="ppc64"
            fi
            ;;
    esac

    echo "${qarch}"
}
