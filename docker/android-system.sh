set -ex

main() {
    local arch=$1
    local td=$(mktemp -d)
    pushd $td

    local dependencies=(
        ca-certificates
        curl
        gcc-multilib
        git
        g++-multilib
        make
        python
    )

    # fake java and javac, it is not necessary for what we build, but the build
    # script ask for it
    cat << EOF > /usr/bin/java
#!/bin/bash
echo "java version \"1.7.0\""
echo "OpenJDK Runtime Environment (IcedTea 2.6.9)"
echo "OpenJDK 64-Bit Server VM (build 24.131-b00, mixed mode)"
EOF

    cat << EOF > /usr/bin/javac
#!/bin/bash
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
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    curl -O https://storage.googleapis.com/git-repo-downloads/repo
    chmod +x repo

    # this is the minimum set of modules that are need to build bionic
    # this was created by trial and error
    ./repo init -u https://android.googlesource.com/platform/manifest -b android-5.0.0_r1
    ./repo sync bionic
    ./repo sync build
    ./repo sync external/compiler-rt
    ./repo sync external/jemalloc
    ./repo sync external/libcxx
    ./repo sync external/libcxxabi
    ./repo sync external/stlport
    ./repo sync prebuilts/clang/linux-x86/host/3.5
    ./repo sync system/core
    case $arch in
        arm)
            ./repo sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.8
        ;;
        arm64)
            ./repo sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.8
            ./repo sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            ./repo sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.8
        ;;
        x86_64)
            ./repo sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.8
        ;;
    esac

    # avoid build tests
    rm bionic/linker/tests/Android.mk bionic/tests/Android.mk

    source build/envsetup.sh
    lunch aosp_$arch-user
    mmma bionic/

    if [ $arch = "arm" ]; then
        mv out/target/product/generic/system/ /
    else
        mv out/target/product/generic_$arch/system/ /
    fi

    # clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    popd

    rm -rf $td
    rm $0
}

main "${@}"
