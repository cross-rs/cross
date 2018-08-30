set -ex

main() {
    local binutils=2.25.1 \
          gcc=5.3.0 \
          target=x86_64-unknown-netbsd

    local dependencies=(
        bzip2
        ca-certificates
        curl
        g++
        make
        patch
        wget
        xz-utils
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

    mkdir $td/{binutils,gcc}{,-build} $td/netbsd

    curl https://ftp.gnu.org/gnu/binutils/binutils-$binutils.tar.bz2 | \
        tar -C $td/binutils --strip-components=1 -xj

    curl https://ftp.gnu.org/gnu/gcc/gcc-$gcc/gcc-$gcc.tar.bz2 | \
        tar -C $td/gcc --strip-components=1 -xj

    pushd $td

    cd gcc
    ./contrib/download_prerequisites
    local patches=(
        ftp://ftp.netbsd.org/pub/pkgsrc/stable/pkgsrc/lang/gcc5/patches/patch-libstdc++-v3_config_os_bsd_netbsd_ctype__base.h
        ftp://ftp.netbsd.org/pub/pkgsrc/stable/pkgsrc/lang/gcc5/patches/patch-libstdc++-v3_config_os_bsd_netbsd_ctype__configure__char.cc
    )

    local patch
    for patch in ${patches[@]}; do
        patch=$(curl $patch)
        echo "$patch" | patch -Np0
    done
    cd ..

    curl ftp://ftp.netbsd.org/pub/NetBSD/NetBSD-7.0/amd64/binary/sets/base.tgz | \
        tar -C $td/netbsd -xz ./usr/include ./usr/lib ./lib

    curl ftp://ftp.netbsd.org/pub/NetBSD/NetBSD-7.0/amd64/binary/sets/comp.tgz | \
        tar -C $td/netbsd -xz ./usr/include ./usr/lib

    cd binutils-build
    ../binutils/configure \
        --target=$target
    nice make -j$(nproc)
    make install
    cd ..

    local destdir=/usr/local/$target
    cp -r $td/netbsd/usr/include $destdir/
    cp $td/netbsd/lib/libc.so.12.193.1 $destdir/lib
    cp $td/netbsd/lib/libm.so.0.11 $destdir/lib
    cp $td/netbsd/lib/libutil.so.7.21 $destdir/lib
    cp $td/netbsd/usr/lib/libpthread.so.1.2 $destdir/lib
    cp $td/netbsd/usr/lib/librt.so.1.1 $destdir/lib
    cp $td/netbsd/usr/lib/lib{c,m,pthread}{,_p,_pic}.a $destdir/lib
    cp $td/netbsd/usr/lib/{crt0,crti,crtn,crtbeginS,crtendS,crtbegin,crtend,gcrt0}.o $destdir/lib

    ln -s libc.so.12.193.1 $destdir/lib/libc.so
    ln -s libc.so.12.193.1 $destdir/lib/libc.so.12
    ln -s libm.so.0.11 $destdir/lib/libm.so
    ln -s libm.so.0.11 $destdir/lib/libm.so.0
    ln -s libpthread.so.1.2 $destdir/lib/libpthread.so
    ln -s libpthread.so.1.2 $destdir/lib/libpthread.so.1
    ln -s librt.so.1.1 $destdir/lib/librt.so
    ln -s libutil.so.7.21 $destdir/lib/libutil.so
    ln -s libutil.so.7.21 $destdir/lib/libutil.so.7

    cd gcc-build
    ../gcc/configure \
        --disable-libada \
        --disable-libcilkrt \
        --disable-libcilkrts \
        --disable-libgomp \
        --disable-libquadmath \
        --disable-libquadmath-support \
        --disable-libsanitizer \
        --disable-libssp \
        --disable-libvtv \
        --disable-lto \
        --disable-multilib \
        --disable-nls \
        --enable-languages=c,c++ \
        --target=$target
    nice make -j$(nproc)
    make install
    cd ..

    # clean up
    popd

    apt-get purge --auto-remove -y ${purge_list[@]}

    rm -rf $td
    rm $0
}

main "${@}"
