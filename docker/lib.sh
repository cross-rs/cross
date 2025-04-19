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

find_argument() {
    # Extracts the value from an argument of the form VARIABLE=VALUE
    local needle="$1"
    local return_var="$2"
    shift 2
    local prefix="${needle}="
    for var in "${@}"; do
        case "$var" in
            "$prefix"*)
                eval "$return_var=${var#"${prefix}"}"
                return 0 ;;
            *)           ;;
        esac
    done
    echo "Missing argument ${needle}"
    exit 1
}

symlinkify_if_same() {
    local file1="$1"
    local file2="$2"
    # Only make a symlink if the files are identical, and the destination file isn't already a symlink
    if [ ! -L "${file2}" ] && cmp "$file1" "$file2"; then
        ln -sf "$file1" "$file2"
    fi
}

symlinkify_and_strip_toolchain() {
    local target="$1"
    local gcc_ver="$2"

    local target_bin="/usr/local/${target}/bin"
    local local_bin="/usr/local/bin"

    # The first set of tools appear as /usr/local/bin/<target>-<tool> and /usr/local/<target>/bin/<tool>

    # Special case: ld is itself usually hardlinked to ld.bfd
    symlinkify_if_same "${local_bin}/ld" "${local_bin}/ld.bfd"

    # Turn hard links or otherwise identical files into symlinks
    for tool in ar  as  ld  ld.bfd  nm  objcopy  objdump  ranlib  readelf  strip; do
        local src="${local_bin}/${target}-${tool}"
        local dest="${target_bin}/${tool}"
        symlinkify_if_same "${src}" "${dest}"
        strip "${src}"
    done

    # The second set of tools only appear as /usr/local/bin/<target>-<tool>

    # Special case: c++ and g++ are usually the same file
    symlinkify_if_same "${local_bin}/${target}-c++" "${local_bin}/${target}-g++"
    # Special case: gcc and gcc-<version>
    symlinkify_if_same "${local_bin}/${target}-gcc" "${local_bin}/${target}-gcc-${gcc_ver}"

    for tool in  addr2line  c++ c++filt  cpp  elfedit  g++  gcc  gcc-${gcc_ver}  gcc-ar  gcc-nm  gcc-ranlib  gcov  gcov-dump  gcov-tool  gfortran  gprof  size  strings; do
        strip "${local_bin}/${target}-${tool}"
    done
}
