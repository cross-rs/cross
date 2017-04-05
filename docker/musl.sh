set -ex

main() {
    local version=$1 \
          target=$2

    local dependencies=(
        ca-certificates
        curl
        make
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
    curl https://www.musl-libc.org/releases/musl-$version.tar.gz | \
        tar --strip-components=1 -xz

    if [ ! -z $target ]; then
        ln -s /usr/bin/{,$target-}ar
        ln -s /usr/bin/{,$target-}cc
        ln -s /usr/bin/{,$target-}ranlib
    fi

    CFLAGS="-fPIC ${@:3}" ./configure \
          --disabled-shared \
          --prefix=/usr/local \
          $(test -z $target || echo --target=$target)
    nice make -j$(nproc)
    nice make install
    ln -s /usr/bin/ar /usr/local/bin/musl-ar

    if [ ! -z $target ]; then
        rm /usr/bin/$target-{ar,ranlib}
    fi

    # clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
