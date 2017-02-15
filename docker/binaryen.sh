set -ex

main() {
    local version=$1

    local dependencies=(
        ca-certificates
        cmake
        curl
        g++
        ninja-build
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

    curl -L https://github.com/WebAssembly/binaryen/archive/$version.tar.gz | \
        tar -C $td --strip-components=1 -xz

    pushd $td
    cmake -G Ninja
    nice ninja

    mkdir /binaryen
    cp -r bin lib src /binaryen
    cp -r src/js /binaryen/src

    # Cleanup
    popd

    apt-get purge --auto-remove -y ${purge_list[@]}

    rm -rf $td
    rm $0
}

main "${@}"
