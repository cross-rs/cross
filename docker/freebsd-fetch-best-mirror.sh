#!/bin/bash
set -e

# shellcheck disable=SC1091
. /freebsd-install.sh
fetch_best_freebsd_mirror "$@"
