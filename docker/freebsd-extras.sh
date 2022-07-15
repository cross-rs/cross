#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh
# shellcheck disable=SC1091
. freebsd-common.sh
# shellcheck disable=SC1091
. freebsd-install.sh

main() {
    setup_packagesite
    install_freebsd_package openssl sqlite3

    rm "${0}"
}

main "${@}"
