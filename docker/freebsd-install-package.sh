#!/bin/bash
set -e

# shellcheck disable=SC1091
. /freebsd-install.sh
install_freebsd_package "$@"
