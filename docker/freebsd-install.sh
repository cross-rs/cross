#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. /freebsd-common.sh

# list of SRV records to query if the default mirror fails
FREEBSD_HTTP_TCP_SOURCES=(
    # these return all mirrors, including local ones
    "_http._tcp.pkg.all.freebsd.org"
    # this only returns geodns mirrors
    "_http._tcp.pkg.freebsd.org"
)
FREEBSD_PACKAGEDIR="/opt/freebsd-packagesite"
FREEBSD_PACKAGESITE="${FREEBSD_PACKAGEDIR}/packagesite.yaml"
FREEBSD_TARGET="${ARCH}-unknown-freebsd${FREEBSD_MAJOR}"
FREEBSD_DEFAULT_MIRROR="pkg.freebsd.org"
# NOTE: these mirrors were known to work as of 2022-11-28.
# no availability guarantees are made for any of them.
FREEBSD_BACKUP_MIRRORS=(
    "pkg0.syd.freebsd.org"
    "pkg0.bme.freebsd.org"
    "pkg0.bra.freebsd.org"
    "pkg0.fra.freebsd.org"
    "pkg0.jinx.freebsd.org"
    "pkg0.kul.freebsd.org"
    "pkg0.kwc.freebsd.org"
    "pkg0.nyi.freebsd.org"
    "pkg0.tuk.freebsd.org"
    "pkg0.twn.freebsd.org"
)

# NOTE: out of convention, we use `url` for mirrors with the scheme,
# and `mirror` for those without the scheme for consistent naming.
freebsd_package_source() {
    local url="${1}"
    echo "${url}/FreeBSD:${FREEBSD_MAJOR}:${FREEBSD_ARCH}/quarterly"
}

freebsd_mirror_works() {
    local mirror="${1}"
    local scheme="${2}"
    local pkg_source=

    # meta.conf is a small file for quick confirmation the mirror works
    pkg_source=$(freebsd_package_source "${scheme}://${mirror}")
    local path="${pkg_source}/meta.conf"

    timeout 20s curl --retry 3 -sSfL "${path}" >/dev/null 2>&1
}

_fetch_best_freebsd_mirror() {
    # in case if the default mirror is down, we can use various known
    # fallbacks, or at worst, SRV fallbacks to find the ideal package
    # site. no individual mirror other than the default mirror is
    # guaranteed to exist, so we use a tiered approach. only
    # the default mirror supports https.
    if freebsd_mirror_works "${FREEBSD_DEFAULT_MIRROR}" "https"; then
        echo "https://${FREEBSD_DEFAULT_MIRROR}"
        return 0
    fi

    # if we've gotten here, it could be a DNS issue, so using a DNS
    # resolver to fetch SRV fallbacks may not work. let's first try
    # a few previously tested mirrors and see if any work.
    local mirror=
    for mirror in "${FREEBSD_BACKUP_MIRRORS[@]}"; do
        if freebsd_mirror_works "${mirror}" "http"; then
            echo "http://${mirror}"
            return 0
        fi
    done

    local http_tcp_source=
    local response=
    local lines=
    # shellcheck disable=SC2016
    local regex='/\d+\s+\d+\s+\d+\s+(.*)\./; print $1'
    for http_tcp_source in "${FREEBSD_HTTP_TCP_SOURCES[@]}"; do
        # the output will have the following format, but we only want the
        # target and ignore everything else:
        #   $priority $port $weight $target.
        #
        # some output may not match, so we skip those lines, for example:
        #   96.47.72.71
        response=$(dig +short srv "${http_tcp_source}")
        readarray -t lines <<< "${response}"
        for line in "${lines[@]}"; do
            mirror=$(echo "${line}" | perl -nle "${regex}")
            if [[ -n "${mirror}" ]]; then
                if freebsd_mirror_works "${mirror}" "http"; then
                    echo "http://${mirror}"
                    return 0
                fi
            fi
        done
    done

    echo -e "\e[31merror:\e[0m could not find a working FreeBSD package mirror." 1>&2
    exit 1
}

fetch_best_freebsd_mirror() {
    set +e
    _fetch_best_freebsd_mirror
    code=$?
    set -e

    return "${code}"
}

setup_freebsd_packagesite() {
    local url="${FREEBSD_MIRROR:-}"
    local pkg_source=

    if [[ -z "${url}" ]]; then
        url=$(fetch_best_freebsd_mirror)
    fi
    pkg_source=$(freebsd_package_source "${url}")

    mkdir -p "${FREEBSD_PACKAGEDIR}"
    curl --retry 3 -sSfL "${pkg_source}/packagesite.txz" -O
    tar -C "${FREEBSD_PACKAGEDIR}" -xJf packagesite.txz

    rm packagesite.txz
}

# don't provide the mirror as a positional argument, so it can be optional
install_freebsd_package() {
    local url="${FREEBSD_MIRROR:-}"
    local pkg_source=
    local name
    local path
    local pkg
    local td
    local destdir="/usr/local/${FREEBSD_TARGET}"

    if [[ -z "${url}" ]]; then
        url=$(fetch_best_freebsd_mirror)
    fi
    pkg_source=$(freebsd_package_source "${url}")

    td="$(mktemp -d)"
    pushd "${td}"

    for name in "${@}"; do
        path=$(jq -c '. | select ( .name == "'"${name}"'" ) | .repopath' "${FREEBSD_PACKAGESITE}")
        if [[ -z "${path}" ]]; then
            echo "Unable to find package ${name}" >&2
            exit 1
        fi
        path=${path//'"'/}
        pkg=$(basename "${path}")

        mkdir "${td}"/package
        curl --retry 3 -sSfL "${pkg_source}/${path}" -O
        tar -C "${td}/package" -xJf "${pkg}"
        cp -r "${td}/package/usr/local"/* "${destdir}"/

        rm "${td:?}/${pkg}"
        rm -rf "${td:?}/package"
    done

    # clean up
    popd
    rm -rf "${td:?}"
}
