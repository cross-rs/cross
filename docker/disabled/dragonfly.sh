#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local binutils=2.25.1 \
          dragonfly=4.6.1_REL \
          gcc=5.3.0 \
          target=x86_64-unknown-dragonfly

    local dependencies=(
        bsdtar
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
            apt-get install --no-install-recommends --assume-yes $dep
            purge_list+=( $dep )
        fi
    done

    local td=$(mktemp -d)

    mkdir $td/{binutils,gcc}{,-build} $td/dragonfly

    curl https://ftp.gnu.org/gnu/binutils/binutils-$binutils.tar.bz2 | \
        tar -C $td/binutils --strip-components=1 -xj

    curl https://ftp.gnu.org/gnu/gcc/gcc-$gcc/gcc-$gcc.tar.bz2 | \
        tar -C $td/gcc --strip-components=1 -xj

    pushd $td

    cd gcc
    sed -i -e 's/ftp:/https:/g' ./contrib/download_prerequisites
    ./contrib/download_prerequisites
    patch -p0 <<'EOF'
--- libatomic/configure.tgt.orig	2015-07-09 16:08:55 UTC
+++ libatomic/configure.tgt
@@ -110,7 +110,7 @@ case "${target}" in
 	;;

   *-*-linux* | *-*-gnu* | *-*-k*bsd*-gnu \
-  | *-*-netbsd* | *-*-freebsd* | *-*-openbsd* \
+  | *-*-netbsd* | *-*-freebsd* | *-*-openbsd* | *-*-dragonfly* \
   | *-*-solaris2* | *-*-sysv4* | *-*-irix6* | *-*-osf* | *-*-hpux11* \
   | *-*-darwin* | *-*-aix* | *-*-cygwin*)
 	# POSIX system.  The OS is supported.
EOF

    patch -p0 <<'EOF'
--- libstdc++-v3/config/os/bsd/dragonfly/os_defines.h.orig	2015-07-09 16:08:54 UTC
+++ libstdc++-v3/config/os/bsd/dragonfly/os_defines.h
@@ -29,4 +29,9 @@
 // System-specific #define, typedefs, corrections, etc, go here.  This
 // file will come before all others.

+#define _GLIBCXX_USE_C99_CHECK 1
+#define _GLIBCXX_USE_C99_DYNAMIC (!(__ISO_C_VISIBLE >= 1999))
+#define _GLIBCXX_USE_C99_LONG_LONG_CHECK 1
+#define _GLIBCXX_USE_C99_LONG_LONG_DYNAMIC (_GLIBCXX_USE_C99_DYNAMIC || !defined __LONG_LONG_SUPPORTED)
+
 #endif
EOF

    patch -p0 <<'EOF'
--- libstdc++-v3/configure.orig	2016-05-26 18:34:47.163132921 +0200
+++ libstdc++-v3/configure	2016-05-26 18:35:29.594590648 +0200
@@ -52013,7 +52013,7 @@

     ;;

-  *-freebsd*)
+  *-freebsd* | *-dragonfly*)
     SECTION_FLAGS='-ffunction-sections -fdata-sections'


EOF
    cd ..

    curl https://mirror-master.dragonflybsd.org/iso-images/dfly-x86_64-$dragonfly.iso.bz2 | \
        bzcat | \
        bsdtar xf - -C $td/dragonfly ./usr/include ./usr/lib ./lib

    cd binutils-build
    ../binutils/configure \
        --target=$target
    make -j$(nproc)
    make install
    cd ..

    local destdir=/usr/local/$target
    cp -r $td/dragonfly/usr/include $destdir/
    cp $td/dragonfly/lib/libc.so.8 $destdir/lib
    cp $td/dragonfly/lib/libm.so.4 $destdir/lib
    cp $td/dragonfly/lib/libutil.so.4 $destdir/lib
    cp $td/dragonfly/usr/lib/libexecinfo.so.1 $destdir/lib
    cp $td/dragonfly/usr/lib/libpthread.so $destdir/lib/libpthread.so
    cp $td/dragonfly/usr/lib/librt.so.0 $destdir/lib
    cp $td/dragonfly/usr/lib/lib{c,m,util}.a $destdir/lib
    cp $td/dragonfly/usr/lib/thread/libthread_xu.so.2 $destdir/lib/libpthread.so.0
    cp $td/dragonfly/usr/lib/{crt1,Scrt1,crti,crtn}.o $destdir/lib

    ln -s libc.so.8 $destdir/lib/libc.so
    ln -s libexecinfo.so.1 $destdir/lib/libexecinfo.so
    ln -s libm.so.4 $destdir/lib/libm.so
    ln -s librt.so.0 $destdir/lib/librt.so
    ln -s libutil.so.4 $destdir/lib/libutil.so

    cd gcc-build
    ../gcc/configure \
        --disable-libada \
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
    make -j$(nproc)
    make install
    cd ..

    # clean up
    popd

    if (( ${#purge_list[@]} )); then
      apt-get purge --auto-remove -y ${purge_list[@]}
    fi

    rm -rf $td
    rm $0
}

main "${@}"
