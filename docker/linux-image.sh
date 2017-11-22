set -ex

main() {
    local arch=$1 \
          kernel=

    case $arch in
        aarch64)
            arch=arm64
            kernel=4.9.0-4-arm64
            ;;
        *)
            echo "Invalid arch: $arch"
            exit 1
            ;;
    esac

    local dependencies=(
        cpio
        debian-archive-keyring
    )

    local purge_list=()
    apt-get update
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    # Download packages
    mv /etc/apt/sources.list /etc/apt/sources.list.bak
    echo "deb http://http.debian.net/debian/ stretch main contrib non-free" > \
        /etc/apt/sources.list

    dpkg --add-architecture $arch
    apt-get update

    mkdir -p -m 777 /qemu/$arch
    cd /qemu/$arch
    apt-get -t stretch -d --no-install-recommends download \
        busybox:$arch \
        libc6:$arch \
        libgcc1:$arch \
        libssl1*:$arch \
        libstdc++6:$arch \
        linux-image-$kernel:$arch \
        zlib1g:$arch
    cd /qemu

    # Install packages
    root=root-$arch
    mkdir -p $root/{bin,etc,root,sys,dev,proc,sbin,usr/{bin,sbin}}
    for deb in $arch/*deb; do
        dpkg -x $deb $root/
    done

    # kernel
    cp $root/boot/vmlinu* kernel

    # initrd
    mkdir -p $root/modules
    cp \
        $root/lib/modules/*/kernel/drivers/virtio/* \
        $root/lib/modules/*/kernel/fs/9p/9p.ko \
        $root/lib/modules/*/kernel/fs/fscache/fscache.ko \
        $root/lib/modules/*/kernel/net/9p/9pnet.ko \
        $root/lib/modules/*/kernel/net/9p/9pnet_virtio.ko \
        $root/modules || true # some file may not exist
    rm -rf $root/boot
    rm -rf $root/lib/modules

    cat << 'EOF' > $root/init
#!/bin/busybox sh

set -e

/bin/busybox --install

mount -t devtmpfs devtmpfs /dev
mount -t proc none /proc
mount -t sysfs none /sys

# some archs does not have virtio modules
insmod /modules/virtio.ko || true
insmod /modules/virtio_ring.ko || true
insmod /modules/virtio_mmio.ko || true
insmod /modules/virtio_pci.ko || true
insmod /modules/fscache.ko
insmod /modules/9pnet.ko
insmod /modules/9pnet_virtio.ko || true
insmod /modules/9p.ko

mkdir /target
mount -t 9p -o trans=virtio target /target -oversion=9p2000.L || true

echo "emulator is ready! $(cut -d' ' -f1 /proc/uptime)"

exec sh
EOF

    chmod +x $root/init
    cd $root && find . | cpio --create --format='newc' --quiet | gzip > ../initrd.gz

    # Clean up
    rm -rf $root $arch
    mv -f /etc/apt/sources.list.bak /etc/apt/sources.list
    # can fail if arch is used (amd64 and/or i386)
    dpkg --remove-architecture $arch || true
    apt-get update
    apt-get purge --auto-remove -y ${purge_list[@]}
    ls -lh /qemu
}

main "${@}"
