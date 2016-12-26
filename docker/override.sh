set -ex

main() {
    local dependencies=(
        ca-certificates
        curl
    )

    apt-get update

    apt-get install --no-install-recommends -y ${dependencies[@]}

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

    apt-get purge --auto-remove -y ${dependencies[@]}

    rm $0
}

main "${@}"
