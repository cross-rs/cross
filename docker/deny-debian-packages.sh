#!/usr/bin/env bash

set -x
set -euo pipefail

deny_package() {
    local package="${1}"
    local filename="${2}"
    echo "Package: ${package}:${TARGET_ARCH}
Pin: release *
Pin-Priority: -1" > "/etc/apt/preferences.d/${filename}"
}

main() {
    if [[ $# -eq 0 ]]; then
        deny_package '*' "all-packages"
    else
        local package
        for package in "${@}"; do
            deny_package "${package}" "${package}"
            echo "${package}"
        done
    fi

    rm "${0}"
}

main "${@}"
