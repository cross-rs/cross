set -ex

main() {
    local version=2.9.0

    local arch=$1 \
          td=$(mktemp -d)

    local dependencies=(
        autoconf
        automake
        bzip2
        curl
        g++
        libglib2.0-dev
        libtool
        make
        pkg-config
        zlib1g-dev
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    pushd $td

    curl -L http://wiki.qemu-project.org/download/qemu-$version.tar.bz2 | \
        tar --strip-components=1 -xj
    ./configure \
        --disable-kvm \
        --disable-vnc \
        --enable-user \
        --static \
        --target-list=$arch-linux-user
    nice make -j$(nproc)
    make install

    # HACK the binfmt_misc interpreter we'll use expects the QEMU binary to be
    # in /usr/bin. Create an appropriate symlink
    ln -s /usr/local/bin/qemu-$arch /usr/bin/qemu-$arch-static

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
