#!/usr/bin/env bash
# shellcheck disable=SC2294

purge_list=()

install_packages() {
    if grep -i ubuntu /etc/os-release; then
        apt-get update

        for pkg in "${@}"; do
            if ! dpkg -L "${pkg}" >/dev/null 2>/dev/null; then
                apt-get install --assume-yes --no-install-recommends "${pkg}"

                purge_list+=( "${pkg}" )
            fi
        done
    else
        for pkg in "${@}"; do
            if ! yum list installed "${pkg}" >/dev/null 2>/dev/null; then
                yum install -y "${pkg}"

                purge_list+=( "${pkg}" )
            fi
        done
    fi
}

purge_packages() {
    if (( ${#purge_list[@]} )); then
        if grep -i ubuntu /etc/os-release; then
            apt-get purge --assume-yes --auto-remove "${purge_list[@]}"
        else
            yum remove -y "${purge_list[@]}"
        fi
    fi
}

if_centos() {
    if grep -q -i centos /etc/os-release; then
        eval "${@}"
    fi
}

if_ubuntu() {
    if grep -q -i ubuntu /etc/os-release; then
        eval "${@}"
    fi
}

GNU_MIRRORS=(
    "https://ftp.gnu.org/gnu/"
    "https://ftpmirror.gnu.org/"
)

download_mirrors() {
    local relpath="${1}"
    shift
    local filename="${1}"
    shift

    for mirror in "${@}"; do
        if curl --retry 3 -sSfL "${mirror}/${relpath}/${filename}" -O; then
            break
        fi
    done
    if [[ ! -f "${filename}" ]]; then
        echo "Unable to download ${filename}" >&2
        exit 1
    fi
}

download_binutils() {
    local mirror
    local version="${1}"
    local ext="${2}"
    local filename="binutils-${version}.tar.${ext}"

    download_mirrors "binutils" "${filename}" "${GNU_MIRRORS[@]}"
}

download_gcc() {
    local mirror
    local version="${1}"
    local ext="${2}"
    local filename="gcc-${version}.tar.${ext}"

    download_mirrors "gcc/gcc-${version}" "${filename}" "${GNU_MIRRORS[@]}"
}
