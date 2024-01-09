#!/bin/bash
set -e

# shellcheck disable=SC1091
. /freebsd-install.sh
setup_freebsd_packagesite "$@"
