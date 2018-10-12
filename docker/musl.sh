set -ex

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
    curl -L https://github.com/richfelker/musl-cross-make/archive/v0.9.7.tar.gz | \
        tar --strip-components=1 -xz

    # musl-cross-make 0.9.7 does not have musl 1.1.20 hash
    echo "469b3af68a49188c8db4cc94077719152c0d41f1  musl-1.1.20.tar.gz" \
            > hashes/musl-1.1.20.tar.gz.sha1

    nice make install -j$(nproc) \
        GCC_VER=6.3.0 \
        MUSL_VER=1.1.20 \
        DL_CMD="curl -C - -L -o" \
        OUTPUT=/usr/local/ \
        "${@}"

    # clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
