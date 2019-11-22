#!/usr/bin/env bash

set -x
set -euo pipefail

hide_output() {
    set +x
    on_err="
echo ERROR: An error was encountered with the build.
cat /tmp/build.log
exit 1
"
    trap "$on_err" ERR
    bash -c "while true; do sleep 30; echo \$(date) - building ...; done" &
    PING_LOOP_PID=$!
    "$@" &> /tmp/build.log
    trap - ERR
    kill $PING_LOOP_PID
    set -x
}

main() {
    local dependencies=(
        ca-certificates
        curl
        build-essential
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    local td=$(mktemp -d)

    pushd $td
    curl -L https://github.com/richfelker/musl-cross-make/archive/v0.9.8.tar.gz | \
        tar --strip-components=1 -xz

    hide_output make install -j$(nproc) \
        GCC_VER=6.4.0 \
        MUSL_VER=1.1.22 \
        DL_CMD="curl -C - -L -o" \
        OUTPUT=/usr/local/ \
        "$@"

    # clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
