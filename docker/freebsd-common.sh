#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. freebsd-arch.sh

export BSD_ARCH=
case "${ARCH}" in
    x86_64)
        BSD_ARCH=amd64
        ;;
    i686)
        BSD_ARCH=i386
        ;;
esac
export BSD_HOME="ftp.freebsd.org/pub/FreeBSD/releases"
export BSD_MAJOR=12
