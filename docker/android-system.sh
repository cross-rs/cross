#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local arch="${1}"
    local td
    td="$(mktemp -d)"
    pushd "${td}"

    local dependencies=(
        ca-certificates
        curl
        gcc-multilib
        git
        g++-multilib
        libncurses5
        libtinfo5
        make
        openssh-client
        python
        python3
        xz-utils
    )

    # fake java and javac, it is not necessary for what we build, but the build
    # script ask for it
    cat << EOF > /usr/bin/java
#!/usr/bin/env bash
echo "java version \"1.7.0\""
echo "OpenJDK Runtime Environment (IcedTea 2.6.9)"
echo "OpenJDK 64-Bit Server VM (build 24.131-b00, mixed mode)"
EOF

    cat << EOF > /usr/bin/javac
#!/usr/bin/env bash
echo "javac 1.7.0"
EOF

    chmod +x /usr/bin/java
    chmod +x /usr/bin/javac

    # more faking
    export ANDROID_JAVA_HOME=/tmp
    mkdir /tmp/lib/
    touch /tmp/lib/tools.jar

    apt-get update
    local purge_list=(default-jre)
    for dep in "${dependencies[@]}"; do
        if ! dpkg -L "${dep}"; then
            apt-get install --assume-yes --no-install-recommends "${dep}"
            purge_list+=( "${dep}" )
        fi
    done

    curl --retry 3 -sSfL https://storage.googleapis.com/git-repo-downloads/repo -O
    chmod +x repo

    # this is the minimum set of modules that are need to build bionic
    # this was created by trial and error
    python3 ./repo init -u https://android.googlesource.com/platform/manifest -b android-9.0.0_r1

    python3 ./repo sync -c art
    python3 ./repo sync -c bionic
    python3 ./repo sync -c build/make
    python3 ./repo sync -c build/blueprint
    python3 ./repo sync -c build/soong
    python3 ./repo sync -c external/clang
    python3 ./repo sync -c external/compiler-rt
    python3 ./repo sync -c external/elfutils
    python3 ./repo sync -c external/jemalloc
    python3 ./repo sync -c external/libcxx
    python3 ./repo sync -c external/libcxxabi
    python3 ./repo sync -c external/libunwind
    python3 ./repo sync -c external/libunwind_llvm
    python3 ./repo sync -c external/llvm
    python3 ./repo sync -c external/lzma
    python3 ./repo sync -c external/mksh
    python3 ./repo sync -c external/safe-iop
    python3 ./repo sync -c external/valgrind
    python3 ./repo sync -c external/vixl
    python3 ./repo sync -c external/zlib
    python3 ./repo sync -c frameworks/hardware/interfaces
    python3 ./repo sync -c hardware/interfaces
    python3 ./repo sync -c libnativehelper
    python3 ./repo sync -c prebuilts/build-tools
    python3 ./repo sync -c prebuilts/clang/host/linux-x86
    python3 ./repo sync -c prebuilts/clang-tools
    #python3 ./repo sync -c prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
    #python3 ./repo sync -c prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
    python3 ./repo sync -c prebuilts/gcc/linux-x86/host/x86_64-linux-glibc2.15-4.8
    python3 ./repo sync -c prebuilts/go/linux-x86
    python3 ./repo sync -c prebuilts/misc
    python3 ./repo sync -c prebuilts/sdk
    python3 ./repo sync -c system/core
    python3 ./repo sync -c system/libhidl
    python3 ./repo sync -c system/tools/hidl

    case "${arch}" in
        arm)
            python3 ./repo sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
        ;;
        arm64)
            python3 ./repo sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
            python3 ./repo sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            python3 ./repo sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
        x86_64)
            python3 ./repo sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
    esac

    # avoid build tests
    rm bionic/linker/tests/Android.mk
    rm bionic/tests/Android.mk
    rm bionic/tests/Android.bp
    rm bionic/benchmarks/Android.bp
    rm bionic/tests/libs/Android.bp
    rm bionic/tests/headers/Android.bp
    rm bionic/tests/headers/posix/Android.bp

    sed -i -z -e 's/cc_test {.*}//g' bionic/libc/malloc_debug/Android.bp
    sed -i -z -e 's/cc_test {.*}//g' bionic/libc/malloc_hooks/Android.bp
    sed -i -z -e 's/cc_test_host {.*}//g' bionic/tools/relocation_packer/Android.bp

    export ALLOW_MISSING_DEPENDENCIES=true

    # patch the linker to avoid the error
    # FATAL: kernel did not supply AT_SECURE
    #sed -i -e 's/if (!kernel_supplied_AT_SECURE)/if (false)/g' bionic/linker/linker_environ.cpp

    set +u
    # shellcheck disable=SC1091
    source build/envsetup.sh
    lunch "aosp_${arch}-user"
    mmma bionic/
    mmma external/mksh/
    mmma system/core/toolbox/
    set -u

    if [[ "${arch}" = "arm" ]]; then
        mv out/target/product/generic/system/ /
    else
        mv "out/target/product/generic_${arch}/system"/ /
    fi

    # list from https://elinux.org/Android_toolbox
    for tool in cat chmod chown cmp cp ctrlaltdel date df dmesg du \
        hd id ifconfig iftop insmod ioctl ionice kill ln log ls \
        lsmod lsof lsusb md5 mkdir mount mv nandread netstat notify \
        printenv ps reboot renice rm rmdir rmmod route schedtop sendevent \
        setconsole setprop sleep smd start stop sync top touch umount \
        uptime vmstat watchprops wipe; do
        ln -s /system/bin/toolbox "/system/bin/${tool}"
    done

    echo "127.0.0.1 localhost" > /system/etc/hosts

    if (( ${#purge_list[@]} )); then
      apt-get purge --auto-remove -y "${purge_list[@]}"
    fi

    popd

    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
