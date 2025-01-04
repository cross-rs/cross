#!/usr/bin/env bash
# shellcheck disable=SC2294

purge_list=()

set_centos_ulimit() {
    # this is a bug affecting buildkit with yum when ulimit is unlimited
    # https://github.com/docker/buildx/issues/379#issuecomment-1196517905
    ulimit -n 1024000
}

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
        set_centos_ulimit
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

if_ubuntu_ge() {
    if grep -q -i ubuntu /etc/os-release; then
        local ver
        ver="$(source /etc/os-release; echo $VERSION_ID)"
        if dpkg --compare-versions "$ver" "ge" "$1"; then
            shift
            eval "${@}"
        fi
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

docker_to_qemu_arch() {
    local arch="${1}"
    case "${arch}" in
        arm64)
            echo "aarch64"
            ;;
        386)
            echo "i386"
            ;;
        amd64)
            echo "x86_64"
            ;;
        arm|ppc64le|riscv64|s390x)
            echo "${arch}"
            ;;
        *)
            echo "Unknown Docker image architecture, got \"${arch}\"." >&2
            exit 1
            ;;
    esac
}

docker_to_linux_arch() {
    # variant may not be provided
    local oldstate
    oldstate="$(set +o)"
    set +u

    local arch="${1}"
    local variant="${2}"
    case "${arch}" in
        arm64)
            echo "aarch64"
            ;;
        386)
            echo "i686"
            ;;
        amd64)
            echo "x86_64"
            ;;
        ppc64le)
            echo "powerpc64le"
            ;;
        arm)
            case "${variant}" in
                v6)
                    echo "arm"
                    ;;
                ""|v7)
                    echo "armv7"
                    ;;
                *)
                    echo "Unknown Docker image variant, got \"${variant}\"." >&2
                    exit 1
                    ;;
            esac
            ;;
        riscv64|s390x)
            echo "${arch}"
            ;;
        *)
            echo "Unknown Docker image architecture, got \"${arch}\"." >&2
            exit 1
            ;;
    esac

    eval "${oldstate}"
}
