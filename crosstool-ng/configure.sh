#!/bin/bash
# shellcheck disable=SC2001,SC2207

set -eo pipefail

# script to programmatically generate the ct-ng config files
# this sets the GCC and glibc versions, so we don't have to hardcode
# them on every build.

scriptdir=$(dirname "${BASH_SOURCE[0]}")
scriptdir=$(realpath "${scriptdir}")

# allow overrides for the default toolchain versions
# we want to support RHEL7 for glibc 2.17.0, and keep
# the same gcc and linux versions as the other images.
if [[ -z "${GCC_VERSION}" ]]; then
    GCC_VERSION="8.3.0"
fi
if [[ -z "${GLIBC_VERSION}" ]]; then
    GLIBC_VERSION="2.17.0"
fi
if [[ -z "${LINUX_VERSION}" ]]; then
    LINUX_VERSION="4.19.21"
fi

configure_template() {
    local gcc_major
    local gcc_minor
    local gcc_patch
    local glibc_major
    local glibc_minor
    local glibc_patch
    local linux_major
    local linux_minor
    local linux_patch

    gcc_major=$(echo "$GCC_VERSION" | cut -d '.' -f 1)
    gcc_minor=$(echo "$GCC_VERSION" | cut -d '.' -f 2)
    gcc_patch=$(echo "$GCC_VERSION" | cut -d '.' -f 3)
    glibc_major=$(echo "$GLIBC_VERSION" | cut -d '.' -f 1)
    glibc_minor=$(echo "$GLIBC_VERSION" | cut -d '.' -f 2)
    # shellcheck disable=SC2034
    glibc_patch=$(echo "$GLIBC_VERSION" | cut -d '.' -f 3)
    linux_major=$(echo "$LINUX_VERSION" | cut -d '.' -f 1)
    linux_minor=$(echo "$LINUX_VERSION" | cut -d '.' -f 2)
    linux_patch=$(echo "$LINUX_VERSION" | cut -d '.' -f 3)

    # write out our valid range of gcc values: from 5-10.
    local ct_gcc_v
    ct_gcc_v="CT_GCC_V_${gcc_major}=y\n"
    ct_gcc_v="${ct_gcc_v}# CT_GCC_NO_VERSIONS is not set\n"
    ct_gcc_v="${ct_gcc_v}CT_GCC_VERSION=\"${gcc_major}.${gcc_minor}.${gcc_patch}\"\n"

    # write out our gcc version ranges
    local ct_gcc=""
    local gcc_is_gt_49=
    local gcc_is_ge_49=
    local gcc_is_gt_48=
    local gcc_is_ge_48=
    if [[ "${gcc_major}" -gt "7" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_later_than_7=y\n"
    fi
    if [[ "${gcc_major}" -ge "7" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_7_or_later=y\n"
    fi
    if [[ "${gcc_major}" -gt "6" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_later_than_6=y\n"
    fi
    if [[ "${gcc_major}" -ge "6" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_6_or_later=y\n"
    fi
    if [[ "${gcc_major}" -gt "5" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_later_than_5=y\n"
    fi
    if [[ "${gcc_major}" -ge "5" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_5_or_later=y\n"
    fi
    if [[ "${gcc_major}" -ge "5" ]]; then
        gcc_is_gt_49=1
        gcc_is_ge_49=1
        gcc_is_gt_48=1
        gcc_is_ge_48=1
    elif [[ "${gcc_major}" == "4" ]]; then
        if [[ "${gcc_minor}" -gt "9" ]]; then
            gcc_is_gt_49=1
        fi
        if [[ "${gcc_minor}" -ge "9" ]]; then
            gcc_is_ge_49=1
        fi
        if [[ "${gcc_minor}" -gt "8" ]]; then
            gcc_is_gt_48=1
        fi
        if [[ "${gcc_minor}" -ge "8" ]]; then
            gcc_is_ge_48=1
        fi
    fi
    if [[ -n "${gcc_is_gt_49}" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_later_than_4_9=y\n"
    fi
    if [[ -n "${gcc_is_ge_49}" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_4_9_or_later=y\n"
    fi
    if [[ -n "${gcc_is_gt_48}" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_later_than_4_8=y\n"
    fi
    if [[ -n "${gcc_is_ge_48}" ]]; then
        ct_gcc="${ct_gcc}CT_GCC_4_8_or_later=y\n"
    fi

    # write out our valid range of glibc values.
    if [[ "${glibc_major}" != "2" ]]; then
        echo "glibc major versions other than 2 currently unsupported, got ${glibc_major}." 2>&1
        exit 1
    fi
    local ct_glibc_v
    ct_glibc_v="CT_GLIBC_V_${glibc_major}_${glibc_minor}=y\n"
    ct_glibc_v="${ct_glibc_v}# CT_GLIBC_NO_VERSIONS is not set\n"
    ct_glibc_v="${ct_glibc_v}CT_GLIBC_VERSION=\"${glibc_major}.${glibc_minor}\"\n"

    # write out our glibc version ranges
    local ct_glibc=""
    if [[ "${glibc_minor}" -le "29" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_29_or_older=y\n"
    fi
    if [[ "${glibc_minor}" -lt "29" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_older_than_2_29=y\n"
    fi
    if [[ "${glibc_minor}" -le "27" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_27_or_older=y\n"
    fi
    if [[ "${glibc_minor}" -lt "27" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_older_than_2_27=y\n"
    fi
    if [[ "${glibc_minor}" -le "26" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_26_or_older=y\n"
    fi
    if [[ "${glibc_minor}" -lt "26" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_older_than_2_26=y\n"
    fi
    if [[ "${glibc_minor}" -le "25" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_25_or_older=y\n"
    fi
    if [[ "${glibc_minor}" -lt "25" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_older_than_2_25=y\n"
    fi
    if [[ "${glibc_minor}" -le "24" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_24_or_older=y\n"
    fi
    if [[ "${glibc_minor}" -lt "24" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_older_than_2_24=y\n"
    fi
    if [[ "${glibc_minor}" -le "23" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_23_or_older=y\n"
    fi
    if [[ "${glibc_minor}" -lt "23" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_older_than_2_23=y\n"
    fi
    if [[ "${glibc_minor}" -le "20" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_20_or_older=y\n"
    fi
    if [[ "${glibc_minor}" -lt "20" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_older_than_2_20=y\n"
    fi
    if [[ "${glibc_minor}" -ge "17" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_17_or_later=y\n"
    fi
    if [[ "${glibc_minor}" -le "17" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_17_or_older=y\n"
    fi
    if [[ "${glibc_minor}" -gt "14" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_later_than_2_14=y\n"
    fi
    if [[ "${glibc_minor}" -ge "14" ]]; then
        ct_glibc="${ct_glibc}CT_GLIBC_2_14_or_later=y\n"
    fi

    # write out our valid range of linux values.
    local ct_linux_v="CT_LINUX_V_${linux_major}_${linux_minor}=y\n"
    local ct_linux_v="${ct_linux_v}# CT_LINUX_NO_VERSIONS is not set\n"
    local ct_linux_v="${ct_linux_v}CT_LINUX_VERSION=\"${linux_major}.${linux_minor}.${linux_patch}\"\n"

    # write out our linux version ranges
    local ct_linux=""
    if [[ "${linux_major}" -lt "4" ]]; then
        ct_linux="${ct_linux}CT_LINUX_older_than_4_8=y\n"
        ct_linux="${ct_linux}CT_LINUX_4_8_or_older=y\n"
    elif [[ "${linux_major}" == "4" ]] && [[ "${linux_minor}" -lt "8" ]]; then
        ct_linux="${ct_linux}CT_LINUX_older_than_4_8=y\n"
        ct_linux="${ct_linux}CT_LINUX_4_8_or_older=y\n"
    else
        ct_linux="${ct_linux}CT_LINUX_later_than_4_8=y\n"
        ct_linux="${ct_linux}CT_LINUX_4_8_or_later=y\n"
    fi
    if [[ "${linux_major}" -lt "3" ]]; then
        ct_linux="${ct_linux}CT_LINUX_older_than_3_7=y\n"
        ct_linux="${ct_linux}CT_LINUX_3_7_or_older=y\n"
    elif [[ "${linux_major}" == "3" ]] && [[ "${linux_minor}" -lt "7" ]]; then
        ct_linux="${ct_linux}CT_LINUX_older_than_3_7=y\n"
        ct_linux="${ct_linux}CT_LINUX_3_7_or_older=y\n"
    else
        ct_linux="${ct_linux}CT_LINUX_later_than_3_7=y\n"
        ct_linux="${ct_linux}CT_LINUX_3_7_or_later=y\n"
    fi
    if [[ "${linux_major}" -lt "3" ]]; then
        ct_linux="${ct_linux}CT_LINUX_older_than_3_2=y\n"
        ct_linux="${ct_linux}CT_LINUX_3_2_or_older=y\n"
    elif [[ "${linux_major}" == "3" ]] && [[ "${linux_minor}" -lt "7" ]]; then
        ct_linux="${ct_linux}CT_LINUX_older_than_3_2=y\n"
        ct_linux="${ct_linux}CT_LINUX_3_2_or_older=y\n"
    else
        ct_linux="${ct_linux}CT_LINUX_later_than_3_2=y\n"
        ct_linux="${ct_linux}CT_LINUX_3_2_or_later=y\n"
    fi

    # now, replace our variables
    local template
    template=$(cat "${1}")
    template=$(echo "${template}" | sed "s/%CT_GCC_V%/${ct_gcc_v}/")
    template=$(echo "${template}" | sed "s/%CT_GCC%/${ct_gcc}/")
    template=$(echo "${template}" | sed "s/%CT_GLIBC_V%/${ct_glibc_v}/")
    template=$(echo "${template}" | sed "s/%CT_GLIBC%/${ct_glibc}/")
    template=$(echo "${template}" | sed "s/%CT_LINUX_V%/${ct_linux_v}/")
    template=$(echo "${template}" | sed "s/%CT_LINUX%/${ct_linux}/")

    echo "$template"
}

main() {
    local srcdir="${scriptdir}"
    local dstdir="${scriptdir}/../docker/crosstool-config"
    local filename
    local srcfile
    local dstfile
    local config

    for srcfile in "$srcdir"/*".config.in"; do
        filename=$(basename "$srcfile")
        dstfile="${dstdir}/${filename//.in/}"
        config=$(configure_template "${srcfile}")
        echo "$config" > "$dstfile"
    done
}

main
