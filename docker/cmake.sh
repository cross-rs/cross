set -ex

main() {
    local version=$1

    local dependencies=(
        curl
        g++
        make
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        dpkg -L $dep || (
            apt-get install --no-install-recommends -y $dep &&
                purge_list+=( $dep )
        )
    done

    local td=$(mktemp -d)

    pushd $td

    curl https://cmake.org/files/v${version%.*}/cmake-$version.tar.gz | \
        tar --strip-components 1 -xz
    ./bootstrap
    nice make -j$(nproc)
    make install

    # clean up
    popd

    apt-get purge --auto-remove -y ${purge_list[@]}

    rm -rf $td
    rm $0
}

main "${@}"
