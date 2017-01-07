set -ex

main() {
    local version=$1

    local dependencies=(
        curl
        g++
        make
    )

    apt-get update
    apt-get install --no-install-recommends -y ${dependencies[@]}

    local td=$(mktemp -d)

    pushd $td

    curl https://cmake.org/files/v${version%.*}/cmake-$version.tar.gz | \
        tar --strip-components 1 -xz
    ./bootstrap
    nice make -j$(nproc)
    make install

    apt-get purge --auto-remove -y ${dependencies[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
