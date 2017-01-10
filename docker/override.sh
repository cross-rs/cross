set -ex

main() {
    local dependencies=(
        ca-certificates
        curl
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        dpkg -L $dep || (
            apt-get install --no-install-recommends -y $dep &&
                purge_list+=( $dep )
        )
    done

    mkdir -p /overrides /.cargo
    echo "paths = [" > /.cargo/config

    local pkg= vers=
    while [ $# -gt 1 ]; do
        pkg=$1
        vers=$2
        shift 2

        mkdir /overrides/$pkg
        curl -L https://japaric.github.io/cross/$pkg-v$vers.tar.gz | \
            tar -C /overrides/$pkg -xz

        echo "\"/overrides/$pkg\"," >> /.cargo/config
    done

    echo "]" >> /.cargo/config

    # clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    rm $0
}

main "${@}"
