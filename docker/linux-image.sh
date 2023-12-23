#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

max_kernel_version() {
    # kernel versions have the following format:
    #   `5.10.0-10-$arch`, where the `$arch` may be optional.
    local IFS=$'\n'
    local -a versions
    local major=0
    local minor=0
    local patch=0
    local release=0
    local index=0
    local version
    local x
    local y
    local z
    local r
    local is_larger

    read -r -d '' -a versions <<< "$1"
    for i in "${!versions[@]}"; do
        version="${versions[$i]}"
        x=$(echo "$version" | cut -d '.' -f 1)
        y=$(echo "$version" | cut -d '.' -f 2)
        z=$(echo "$version" | cut -d '.' -f 3 | cut -d '-' -f 1)
        r=$(echo "$version" | cut -d '-' -f 2)
        is_larger=

        if [ "$x" -gt "$major" ]; then
            is_larger=1
        elif [ "$x" -eq "$major" ] && [ "$y" -gt "$minor" ]; then
            is_larger=1
        elif [ "$x" -eq "$major" ] && [ "$y" -eq "$minor" ] && [ "$z" -gt "$patch" ]; then
            is_larger=1
        elif [ "$x" -eq "$major" ] && [ "$y" -eq "$minor" ] && [ "$z" -eq "$patch" ] && [ "$r" -gt "$release" ]; then
            is_larger=1
        fi

        if [ -n "$is_larger" ]; then
            index="$i"
            major="$x"
            minor="$y"
            patch="$z"
            release="$r"
        fi
    done

    echo "${versions[index]}"
}

main() {
    # arch in the rust target
    local arch="${1}" \
        kversion=5.10.0-26

    local debsource="deb http://http.debian.net/debian/ bullseye main"
    debsource="${debsource}\ndeb http://security.debian.org/ bullseye-security main"

    local dropbear="dropbear-bin"

    local -a deps
    local kernel=
    local libgcc="libgcc-s1"
    local ncurses=

    # select debian arch and kernel version
    case "${arch}" in
        aarch64)
            arch=arm64
            kernel="${kversion}-arm64"
            deps=(libcrypt1:"${arch}")
            ;;
        armv7)
            arch=armhf
            kernel='5.*-armmp'
            deps=(libcrypt1:"${arch}")
            ;;
        i686)
            arch=i386
            kernel="${kversion}-686"
            deps=(libcrypt1:"${arch}")
            ;;
        mips)
            # mips was discontinued in bullseye, so we have to use buster.
            libgcc="libgcc1"
            debsource="deb http://http.debian.net/debian/ buster main"
            debsource="${debsource}\ndeb http://security.debian.org/ buster/updates main"
            kernel='4.*-4kc-malta'
            ncurses="=6.1*"
            ;;
        mipsel)
            kernel='5.*-4kc-malta'
            deps=(libcrypt1:"${arch}")
            ;;
        mips64el)
            kernel='5.*-5kc-malta'
            deps=(libcrypt1:"${arch}")
            ;;
        powerpc)
            # there is no buster powerpc port, so we use jessie
            # use a more recent kernel from backports
            kversion='4.9.0-0.bpo.6'
            kernel="${kversion}-powerpc"
            debsource="deb http://archive.debian.org/debian jessie main"
            debsource="${debsource}\ndeb http://archive.debian.org/debian jessie-backports main"
            debsource="${debsource}\ndeb http://ftp.ports.debian.org/debian-ports unstable main"
            debsource="${debsource}\ndeb http://ftp.ports.debian.org/debian-ports unreleased main"

            # archive.debian.org Release files are expired.
            echo "Acquire::Check-Valid-Until false;" | tee -a /etc/apt/apt.conf.d/10-nocheckvalid
            echo "APT::Get::AllowUnauthenticated true;" | tee -a /etc/apt/apt.conf.d/10-nocheckvalid
            echo "Acquire::AllowInsecureRepositories True;" | tee -a /etc/apt/apt.conf.d/10-nocheckvalid

            dropbear="dropbear"
            deps=(libcrypt1:"${arch}")
            ;;
        powerpc64)
            # there is no stable port
            arch=ppc64
            # https://packages.debian.org/en/sid/linux-image-powerpc64
            kernel='6.*-powerpc64'
            debsource="deb http://ftp.ports.debian.org/debian-ports unstable main"
            debsource="${debsource}\ndeb http://ftp.ports.debian.org/debian-ports unreleased main"
            # sid version of dropbear requires these dependencies
            deps=(libcrypt1:"${arch}")
            ;;
        powerpc64le)
            arch=ppc64el
            kernel='5.*-powerpc64le'
            deps=(libcrypt1:"${arch}")
            ;;
        riscv64)
            kernel='6.*-riscv64'
            debsource="deb http://ftp.ports.debian.org/debian-ports unstable main"
            debsource="${debsource}\ndeb http://ftp.ports.debian.org/debian-ports unreleased main"
            deps=(libcrypt1:"${arch}")
            ;;
        s390x)
            arch=s390x
            kernel='5.*-s390x'
            deps=(libcrypt1:"${arch}")
            ;;
        sparc64)
            # there is no stable port
            # https://packages.debian.org/en/sid/linux-image-sparc64
            kernel='6.*-sparc64'
            debsource="deb http://ftp.ports.debian.org/debian-ports unstable main"
            debsource="${debsource}\ndeb http://ftp.ports.debian.org/debian-ports unreleased main"
            # sid version of dropbear requires these dependencies
            deps=(libcrypt1:"${arch}")
            ;;
        x86_64)
            arch=amd64
            kernel="${kversion}-amd64"
            deps=(libcrypt1:"${arch}")
            ;;
        *)
            echo "Invalid arch: ${arch}"
            exit 1
            ;;
    esac

    install_packages ca-certificates \
        curl \
        cpio \
        sharutils \
        gnupg

    # conflicting versions of some packages will be installed already for the host platform,
    # we need to remove the system installs later. since apt relies
    # on these packages, we need to download them and reinstall
    # using dpkg later, since we cannot redownload via apt.
    local dpkg_arch
    dpkg_arch=$(dpkg --print-architecture)
    local libgcc_packages=("${libgcc}:${arch}" "libstdc++6:${arch}")
    if [[ "${arch}" == "${dpkg_arch}" ]]; then
        local libgcc_root=/qemu/libgcc
        mkdir -p "${libgcc_root}"
        pushd "${libgcc_root}"
        apt-get -d --no-install-recommends download "${libgcc_packages[@]}"
        popd
    fi

    # Download packages
    mv /etc/apt/sources.list /etc/apt/sources.list.bak
    mv /etc/apt/sources.list.d /etc/apt/sources.list.d.bak
    echo -e "${debsource}" > /etc/apt/sources.list

    # Old ubuntu does not support --add-architecture, so we directly change multiarch file
    if [ -f /etc/dpkg/dpkg.cfg.d/multiarch ]; then
        cp /etc/dpkg/dpkg.cfg.d/multiarch /etc/dpkg/dpkg.cfg.d/multiarch.bak
    fi
    dpkg --add-architecture "${arch}" || echo "foreign-architecture ${arch}" > /etc/dpkg/dpkg.cfg.d/multiarch

    # Add Debian keys.
    curl --retry 3 -sSfL 'https://ftp-master.debian.org/keys/archive-key-{7.0,8,9,10,11}.asc' -O
    curl --retry 3 -sSfL 'https://ftp-master.debian.org/keys/archive-key-{8,9,10,11}-security.asc' -O
    curl --retry 3 -sSfL 'https://ftp-master.debian.org/keys/release-{7,8,9,10,11}.asc' -O
    curl --retry 3 -sSfL 'https://www.ports.debian.org/archive_{2020,2021,2022,2023}.key' -O

    for key in *.asc *.key; do
        apt-key add "${key}"
        rm "${key}"
    done

    # allow apt-get to retry downloads
    echo 'APT::Acquire::Retries "3";' > /etc/apt/apt.conf.d/80-retries

    apt-get update

    mkdir -p "/qemu/${arch}"
    chmod 777 /qemu "/qemu/${arch}"

    # Need to limit the kernel version and select the best version
    # if we have a wildcard. This is because some matches, such as
    # `linux-image-4.*-4kc-malta` can match more than 1 package,
    # which will prevent further steps from working.
    if [[ "$kernel" == *'*'* ]]; then
        # Need an exact match for start and end, to avoid debug kernels.
        # Afterwards, need to do a complex sort for the best kernel version,
        # since the sort is non-trivial and must extract subcomponents.
        packages=$(apt-cache search ^linux-image-"$kernel$" --names-only)
        names=$(echo "$packages" | cut -d ' ' -f 1)
        kversions="${names//linux-image-/}"
        kernel=$(max_kernel_version "$kversions")
    fi

    cd "/qemu/${arch}"
    apt-get -d --no-install-recommends download \
        ${deps[@]+"${deps[@]}"} \
        "busybox:${arch}" \
        "${dropbear}:${arch}" \
        "libtommath1:${arch}" \
        "libtomcrypt1:${arch}" \
        "libgmp10:${arch}" \
        "libc6:${arch}" \
        "linux-image-${kernel}:${arch}" \
        ncurses-base"${ncurses}" \
        "zlib1g:${arch}"

    if [[ "${arch}" != "${dpkg_arch}" ]]; then
        apt-get -d --no-install-recommends download "${libgcc_packages[@]}"
    else
        # host arch has conflicting versions of the packages installed
        # this prevents us from downloading them, so we need to
        # simply grab the last version from the debian sources.
        # we're search for a paragraph with:
        #   Maintainer: Debian
        # but not
        #   Original-Maintainer: Debian
        #
        # then, we extract the version record and download **only**
        # packages matching that specific version.
        local version_info
        local version_record
        local version
        for package in "${libgcc_packages[@]}"; do
            version_info=$(apt-cache show "${package}")
            version_record=$(echo "${version_info}" | perl -n00e 'print if /^Maintainer: Debian/m')
            version=$(echo "${version_record}" | grep 'Version: ' | cut -d ' ' -f 2)
            apt-get -d --no-install-recommends download "${package}=${version}"
        done

        # now, if we don't remove the system installs, qemu-system won't
        # be able to find these libgcc packages after building, since it
        # will prefer the system packages, which it can't find later.
        # removing these packages needs to occur after download via apt,
        # since apt-get relies on libgcc_s1 and libstdc++6.
        dpkg -r --force-depends "${libgcc_packages[@]}"
    fi
    cd /qemu

    # Install packages
    root="root-${arch}"
    mkdir -p "${root}"/{bin,etc/dropbear,root,sys,dev,proc,sbin,tmp,usr/{bin,sbin},var/log}
    for deb in "${arch}"/*deb; do
        dpkg -x "${deb}" "${root}"/
    done

    cp "${root}/boot/vmlinu"* kernel

    # initrd
    mkdir -p "${root}/modules"
    cp -v \
        "${root}/lib/modules"/*/kernel/drivers/net/net_failover.ko \
        "${root}/lib/modules"/*/kernel/drivers/net/virtio_net.ko \
        "${root}/lib/modules"/*/kernel/drivers/virtio/* \
        "${root}/lib/modules"/*/kernel/fs/netfs/netfs.ko \
        "${root}/lib/modules"/*/kernel/fs/9p/9p.ko \
        "${root}/lib/modules"/*/kernel/fs/fscache/fscache.ko \
        "${root}/lib/modules"/*/kernel/net/9p/9pnet.ko \
        "${root}/lib/modules"/*/kernel/net/9p/9pnet_virtio.ko \
        "${root}/lib/modules"/*/kernel/net/core/failover.ko \
        "${root}/modules" || true # some file may not exist
    rm -rf "${root:?}/boot"
    rm -rf "${root:?}/lib/modules"

    cat << 'EOF' > "${root}/etc/hosts"
127.0.0.1 localhost qemu
EOF

    cat << 'EOF' > $root/etc/hostname
qemu
EOF

    cat << 'EOF' > $root/etc/passwd
root::0:0:root:/root:/bin/sh
EOF

cat << 'EOF' | uudecode -o $root/etc/dropbear/dropbear_rsa_host_key
begin 600 dropbear_rsa_host_key
M````!W-S:"UR<V$````#`0`!```!`0"N!-<%K,3Z.!Z,OEMB2.N\O.$IWQ*F
M#5%(_;(^2YKY_J_.RQW/7U@_MK&J#!Z0_\;EH#98ZW*E1\.<FF%P/*Y.W56-
M31.'EJE`TN@=T5EC(8"Y%3'ZBYH)^WIVJ]S*G/_;#RH\_?S"U^1L_<<.F`O+
MZVI?*]\KTDOT&QV0#B-M;"%_7:\>+3[X=QMH,B<HM$+0E[\B6*^!XKLR@V,K
M)<V80HHK:_#;D]26XKN&CB./EZAC%4)78R!G""4HT@UK<5I4B^$/""`,?*\T
M>*4$RYULV,V3X6]K:7@Q?80"#WXGGQZNFN6CZ7LTDX(F6J[\]F5<0`HEOF:Z
MX;^53`L'4I/A```!``$L:$Z*#6<^3@+O%.[-#/5H+.C'3\#QQZN[1;J>L`8I
MZ_&T'!"J'/Y+?R?55G:M^=]R*-&I3TOJYZA8@&H51ZOAF59'1_>>Z@?E4#)$
MQU)X/RWH51ZB5KSDWJS:D'7GD(!?NAY`C'7\)I:_4)J")QBV/P"RJQGHG'%B
M1BT2LE6676>`1K,0\NIMZTKQNB(IC+88<7#8%_-=P<&6<"9LH>60TSS?3?-C
MN`T36YB/3^<(Q;`N1NT>I9EZS`BAC^-?.:,R\7EL"<4>7E=]^1]B\K9])AQU
MBM\]M;4V(S(6KH-I.4[6>9E+@\UEM.J6:[2LUEEJDG:G:+:/EVF^Y75@(S$`
M``"!`.O+KW=&*CBCHL"11&SVO4/K]$R-]7MV7,3RR)Q[X'0;6.?4JHW!3VR6
M*FGBY--37ZD-+UV.8_+"$<?B"#&K$.[V)F7V2\UY!7(0FZ@A2`0ADDY*J-_B
M4AU&.*GP#F/!I([:?E],.>6PH9)(/E.\G19#G0K`LRM?JWS!58&;D0C1````
M@0"\[@NYWSTW(?Q@:_A*1Y3/AKYO5?S=0"<2>#V-AH6W-NCSDTSRP=2D79FS
M"D?[;.)V>8'#9&I3"MU@+:2\Z%$0-MG0+J'(0>T1_C6?*C=4U0I$DI<=@D]1
H_&DE8Y(OT%%EPG]!$H&5HX*),_D1A2\P=R.7G'`0L%YM-79Y"T">$0``
`
end
EOF

    # dropbear complains when this file is missing
    touch "${root}/var/log/lastlog"

    cat << 'EOF' > $root/init
#!/bin/busybox sh

set -e

/bin/busybox --install

mount -t devtmpfs devtmpfs /dev
mount -t proc none /proc
mount -t sysfs none /sys
mkdir /dev/pts
mount -t devpts none /dev/pts/

# some archs does not have virtio modules
insmod /modules/failover.ko || true
insmod /modules/net_failover.ko || true
insmod /modules/virtio.ko || true
insmod /modules/virtio_ring.ko || true
insmod /modules/virtio_mmio.ko || true
insmod /modules/virtio_pci_legacy_dev.ko || true
insmod /modules/virtio_pci_modern_dev.ko || true
insmod /modules/virtio_pci.ko || true
insmod /modules/virtio_net.ko || true
insmod /modules/netfs.ko || true
insmod /modules/fscache.ko
insmod /modules/9pnet.ko
insmod /modules/9pnet_virtio.ko || true
insmod /modules/9p.ko

ifconfig lo 127.0.0.1
ifconfig eth0 10.0.2.15
route add default gw 10.0.2.2 eth0

mkdir /target
mount -t 9p -o trans=virtio target /target -oversion=9p2000.u || true

exec dropbear -F -E -B
EOF

    chmod +x "${root}/init"
    cd "${root}"
    find . | cpio --create --format='newc' --quiet | gzip > ../initrd.gz
    cd -

    if [[ "${arch}" == "${dpkg_arch}" ]]; then
        # need to reinstall these packages, since basic utilities rely on them.
        pushd "${libgcc_root}"
        dpkg -i --force-depends "${libgcc_root}"/*.deb
        popd
        rm -rf "${libgcc_root}"
    fi

    # Clean up
    rm -rf "/qemu/${root}" "/qemu/${arch}"
    mv -f /etc/apt/sources.list.bak /etc/apt/sources.list
    mv -f /etc/apt/sources.list.d.bak /etc/apt/sources.list.d
    if [ -f /etc/dpkg/dpkg.cfg.d/multiarch.bak ]; then
        mv /etc/dpkg/dpkg.cfg.d/multiarch.bak /etc/dpkg/dpkg.cfg.d/multiarch
    fi
    if [ -f /etc/apt/apt.conf.d/10-nocheckvalid ]; then
        rm /etc/apt/apt.conf.d/10-nocheckvalid
    fi
    # can fail if arch is used (image arch, such as amd64 and/or i386)
    dpkg --remove-architecture "${arch}" || true
    apt-get update

    # need to reinstall the removed libgcc packages, which are required for apt
    if [[ "${arch}" == "${dpkg_arch}" ]]; then
        apt-get install --no-install-recommends --assume-yes "${packages[@]}"
    fi

    purge_packages

    ls -lh /qemu
}

main "${@}"
