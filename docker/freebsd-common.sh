#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. freebsd-arch.sh

export FREEBSD_ARCH=
case "${ARCH}" in
    x86_64)
        FREEBSD_ARCH=amd64
        ;;
    i686)
        FREEBSD_ARCH=i386
        ;;
esac

export FREEBSD_MAJOR=12
