#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. /freebsd-arch.sh

export FREEBSD_ARCH=
case "${ARCH}" in
    aarch64) # releases are under http://ftp.freebsd.org/pub/FreeBSD/releases/
        FREEBSD_ARCH=arm64 # http://ftp.freebsd.org/pub/FreeBSD/releases/arm64/
        ;;
    x86_64)
        FREEBSD_ARCH=amd64
        ;;
    i686)
        FREEBSD_ARCH=i386
        ;;
esac

export FREEBSD_MAJOR=13
