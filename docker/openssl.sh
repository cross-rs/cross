set -ex

main() {
    local cflags=$4 \
          os=$2 \
          triple=$3 \
          version=$1

    local dependencies=(
        ca-certificates
        curl
        make
        perl
    )

    # NOTE cross toolchain must be already installed
    apt-get update
    apt-get install --no-install-recommends -y ${dependencies[@]}

    td=$(mktemp -d)

    pushd $td
    curl https://www.openssl.org/source/openssl-$version.tar.gz | \
        tar --strip-components=1 -xz
    AR=${triple}ar CC=${triple}gcc ./Configure --prefix=/openssl no-dso $os -fPIC $cflags
    nice make -j$(nproc)
    make install

    apt-get purge --auto-remove -y ${dependencies[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
