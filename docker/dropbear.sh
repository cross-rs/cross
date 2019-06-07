set -ex

main() {
    local version=2019.78 \
          td=$(mktemp -d)

    local dependencies=(
        autoconf
        automake
        bzip2
        curl
        make
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

    curl -L https://matt.ucc.asn.au/dropbear/dropbear-$version.tar.bz2 | \
        tar --strip-components=1 -xj

    # Remove some unwanted message
    sed -i '/skipping hostkey/d' cli-kex.c
    sed -i '/failed to identify current user/d' cli-runopts.c

    ./configure \
       --disable-syslog \
       --disable-shadow \
       --disable-lastlog \
       --disable-utmp \
       --disable-utmpx \
       --disable-wtmp \
       --disable-wtmpx \
       --disable-pututline \
       --disable-pututxline

    nice make -j$(nproc) PROGRAMS=dbclient
    cp dbclient /usr/local/bin/

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
