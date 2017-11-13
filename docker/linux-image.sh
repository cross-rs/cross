set -ex

main() {
    local arch=$1 \
          abi=$2 \
          url_kernel=$3 \
          url_initrd=$4 \
          url_modules=$5

    local dependencies=(
        curl
        cpio
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    mkdir -m 777 /qemu
    cd /qemu

    curl -L $url_kernel -o vmlinuz
    curl -L $url_initrd -o initrd.gz
    curl -L $url_modules -o modules.deb

    # Extract initrd
    mkdir init
    cd init
    gunzip -c ../initrd.gz | cpio -id
    cd -

    # Remove some unecessary modules
    rm -rf init/lib/modules/*/kernel/drivers/net/wireless/
    rm -rf init/lib/modules/*/kernel/drivers/net/ethernet/
    rm -rf init/lib/modules/*/kernel/drivers/net/usb/
    rm -rf init/lib/modules/*/kernel/drivers/usb/
    rm -rf init/lib/modules/*/kernel/drivers/gpu/
    rm -rf init/lib/modules/*/kernel/drivers/mmc/
    rm -rf init/lib/modules/*/kernel/drivers/staging/
    rm -rf init/lib/modules/*/kernel/net/wireless/

    # Copy 9p modules
    dpkg -x modules.deb modules
    cp ./modules/lib/modules/*/kernel/fs/fscache/fscache.ko \
       ./modules/lib/modules/*/kernel/net/9p/9pnet.ko \
       ./modules/lib/modules/*/kernel/net/9p/9pnet_virtio.ko \
       ./modules/lib/modules/*/kernel/fs/9p/9p.ko \
       /qemu/init/lib/modules/
    rm -rf modules

    # Copy libgcc and libstdc++
    cp /usr/$arch-linux-$abi/lib/libgcc_s.so.1 \
       /usr/$arch-linux-$abi/lib/libstdc++.so.6 \
       /qemu/init/usr/lib/

    # Alternative init
    cat << EOF > /qemu/init/init-alt
#!/bin/sh

ip addr add 10.0.2.15/24 dev enp0s1
ip link set enp0s1 up
ip route add default via 10.0.2.2 dev enp0s1

insmod /lib/modules/fscache.ko
insmod /lib/modules/9pnet.ko
insmod /lib/modules/9pnet_virtio.ko
insmod /lib/modules/9p.ko

mkdir /target
mount -t 9p -o trans=virtio target /target -oversion=9p2000.L

exec /bin/sh
EOF

    chmod +x /qemu/init/init-alt

    # Create the new initrd
    cd init
    find . | cpio --create --format='newc' --quiet | gzip > ../initrd.gz
    cd ../
    rm -rf init

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}
}

main "${@}"
