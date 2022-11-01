#!/usr/bin/env bash
# The API level details are mentioned here:
#   https://developer.android.com/studio/releases/platforms
# These are controlled by `ANDROID_VERSION` and `ANDROID_SDK`,
# for example, `ANDROID_SDK=30` and `ANDROID_VERSION=11.0.0_r48`.
#
# You can also build the entire Android source tree with
# `ANDROID_SYSTEM_COMPLETE`, or skip it altogether with
# `ANDROID_SYSTEM_NONE`. Note that runners will not be
# available if the the Android system is not built.
#
# The versions are:
#   5.0: 21 (tested at NDK 10e and r13b, 5.0.0_r1)
#   5.1: 22 (tested at NDK r21d, 5.1.1_r38, unused DT)
#   6.0: 23 (tested at NDK r21dm 6.0.1_r81)
#   7.0: 24 (tested at NDK r21d, 7.0.0_r36)
#   7.1: 25 (tested at NDK r21d, 7.1.2_r39, not supported)
#   8.0: 26 (tested at NDK r21d, 8.0.0_r51)
#   8.1: 27 (tested at NDK r21d, 8.1.0_r81)
#   9.0: 28 (tested at NDK r21d and r25b, 9.0.0_r1)
#   10.0: 29 (tested at NDK r25b, 10.0.0_r47)
#   11.0: 30 (tested at NDK r25b, 11.0.0_r48)
#   12.0: 31 (unable to build at 12.0.0_r34)
#   12.1: 32 (unable to build at 12.1.0_r27)
#   13.0: 33
#
#   API level 25 seems to be missing from Android NDK versions,
#   and therefore is not supported.

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    export ARCH="${1}"
    MAJOR_VERSION=$(echo "${ANDROID_VERSION}" | cut -d '.' -f 1)
    MINOR_VERSION=$(echo "${ANDROID_VERSION}" | cut -d '.' -f 2)
    TAG="android-${ANDROID_VERSION}"

    export MAJOR_VERSION
    export MINOR_VERSION
    export TAG

    if [[ "${ANDROID_SYSTEM_NONE}" == "1" ]]; then
        rm -rf "${PYTHON_TMPDIR}"
        rm "${0}"
        return
    fi

    if [[ "${ANDROID_SYSTEM_COMPLETE}" != "1" ]] && [[ "${MAJOR_VERSION}" -ge 12 ]]; then
        echo "Android versions 12 and higher couple APEX tightly into the build system." 1>&2
        echo "These are currently unsupported, and are unlikely to ever be supported." 1>&2
        echo "Try using a complete Android system build or disable building Android system." 1>&2
        echo "Note that a complete Android system build is slow and creates massive images." 1>&2
        echo "Disabling the Android system build will prevent the use of Android runners." 1>&2
        echo "If you want support for newer versions, contributions are always appreciated." 1>&2
        exit 1
    elif [[ "${MAJOR_VERSION}" -eq 7 ]] && [[ "${MINOR_VERSION}" -eq 1 ]]; then
        echo "Android version 7.1 is not supported." 1>&2
        exit 1
    fi

    local td
    td="$(mktemp -d)"
    pushd "${td}"

    fake_java

    install_packages ca-certificates \
        curl \
        gcc-multilib \
        git \
        g++-multilib \
        libncurses5 \
        libtinfo5 \
        make \
        openssh-client \
        python \
        python3 \
        xz-utils

    curl --retry 3 -sSfL https://storage.googleapis.com/git-repo-downloads/repo -O
    chmod +x repo
    python3 ./repo init -u https://android.googlesource.com/platform/manifest -b "${TAG}"

    local tools=(
        cat chmod chown cmp cp ctrlaltdel date df dmesg du hd id ifconfig
        iftop insmod ioctl ionice kill ln log ls lsmod lsof lsusb md5 mkdir
        mount mv nandread netstat notify printenv ps reboot renice rm rmdir
        rmmod route schedtop sendevent setconsole setprop sleep smd start
        stop sync top touch umount uptime vmstat watchprops wipe
    )
    if [[ "${ANDROID_SYSTEM_COMPLETE}" == "1" ]]; then
        android_repo_complete
    else
        case "${MAJOR_VERSION}" in
            5)
                android_repo_v5
                tools+=(dd getevent getprop grep newfs_msdos)
                ;;
            6)
                android_repo_v6
                ;;
            7)
                android_repo_v7
                ;;
            8)
                android_repo_v8
                ;;
            9)
                android_repo_v9
                ;;
            10)
                android_repo_v10
                ;;
            11)
                android_repo_v11
                ;;
            *)
                echo "Currently unsupported Android version ${MAJOR_VERSION}." 1>&2
                echo "Please submit a feature request if you need support." 1>&2
                exit 1
                ;;
        esac
    fi

    build_android
    install_android "${tools[@]}"

    remove_java
    purge_packages

    popd

    rm -rf "${td}"
    rm -rf "${PYTHON_TMPDIR}"
    rm "${0}"
}

# java isn't required for the build, but the build expects to
# find a java compiler. the supported android versions are:
# https://source.android.com/docs/setup/start/older-versions
#   Android 7: OpenJDK-8
fake_java() {
    local java_type=
    local java_version=
    local jre_info=
    local build_info=

    case "${MAJOR_VERSION}" in
        5|6)
            java_type=java
            java_version=1.7.0
            jre_info="IcedTea 2.6.9"
            build_info="build 24.131-b00, mixed mode"
            ;;
        *)
            java_type=openjdk
            java_version=1.8.0_342
            jre_info="build 1.8.0_342-8u342-b07-0ubuntu1~20.04-b07"
            build_info="build 25.342-b07, mixed mode"
            ;;
    esac

    cat << EOF > /usr/bin/java
#!/usr/bin/env bash
echo "${java_type} version \"${java_version}\""
echo "OpenJDK Runtime Environment (${jre_info})"
echo "OpenJDK 64-Bit Server VM (${build_info})"
EOF

    cat << EOF > /usr/bin/javac
#!/usr/bin/env bash
echo "javac ${java_version}"
EOF

    chmod +x /usr/bin/java
    chmod +x /usr/bin/javac

    # more faking
    export ANDROID_JAVA_HOME=/tmp
    mkdir -p /tmp/lib/
    touch /tmp/lib/tools.jar
}

remove_java() {
    rm /usr/bin/java
    rm /usr/bin/javac
    rm /tmp/lib/tools.jar
}

build_android() {
    if [[ "${ANDROID_SYSTEM_COMPLETE}" != "1" ]]; then
        export ALLOW_MISSING_DEPENDENCIES=true
    fi

    set +u
    # shellcheck disable=SC1091
    source build/envsetup.sh
    lunch "aosp_${ARCH}-user"
    if [[ "${ANDROID_SYSTEM_COMPLETE}" != "1" ]]; then
        mmma bionic/
        mmma external/mksh/
        mmma system/core/toolbox/
    else
        mma
    fi
    if [[ "${ANDROID_SYSTEM_COMPLETE}" != "1" ]] && [[ "${MAJOR_VERSION}" -ge 11 ]]; then
        # for some reason, building bionic doesn't build linker64 on the first pass
        # doing a partial build and a rebuild is just as fast though.
        rm -rf out/target/product/generic
        mmma bionic/
    fi
    set -u
}

install_android() {
    local outdir=
    if [[ "${ARCH}" = "arm" ]]; then
        outdir=out/target/product/generic
    else
        outdir="out/target/product/generic_${ARCH}"
    fi
    mv "${outdir}/system/" /
    if [[ "${ANDROID_SYSTEM_COMPLETE}" == "1" ]] && [[ -d "${outdir}/apex" ]]; then
        # can use the APEX linker, no need to use the bootstrap one
        mv "${outdir}/apex/" /
    elif [[ "${MAJOR_VERSION}" -ge 10 ]]; then
        symlink_bootstrap
    fi

    # list from https://elinux.org/Android_toolbox
    local tool=
    for tool in "${@}"; do
        if [[ ! -f "/system/bin/${tool}" ]]; then
            ln -s /system/bin/toolbox "/system/bin/${tool}"
        fi
    done

    echo "127.0.0.1 localhost" > /system/etc/hosts
}

symlink_bootstrap() {
    # for Android 10+, we need to use the bootstrap linker rather than
    # the APEX linker, which is gigantic. we also symlink the ASAN
    # linker just in case using the bootstrapped one.
    local linker
    local file

    if compgen -G /system/bin/bootstrap/* >/dev/null 2>&1; then
        for linker in /system/bin/bootstrap/*; do
            file=$(basename "${linker}")
            unlink "/system/bin/${file}"
            ln -s "/system/bin/bootstrap/${file}" "/system/bin/${file}"
        done
    fi

    # also need to ensure the shared libraries aren't symlinks
    local lib
    local libdir
    for libdir in /system/lib{,64}; do
        if compgen -G "${libdir}/bootstrap/"* >/dev/null 2>&1; then
            for lib in "${libdir}/bootstrap/"*; do
                file=$(basename "${lib}")
                unlink "${libdir}/${file}"
                ln -s "${libdir}/bootstrap/${file}" "${libdir}/${file}"
            done
        fi
    done
}

# this are the minimum set of modules that are need to build bionic
# this was created by trial and error. this is based on the minimum
# set of modules required for each android version, starting with
# a minimal number of dependencies. for android 10+ versions, we use
# the bootstrap linker rather than the APEX linker for non-complete
# system builds, as the APEX linker drags in nearly the entire Android
# runtime, requiring 60+GB images. for minimal builds, we need to avoid
# APEX altogether, and this gets trickier starting in Android 10
# and much more difficult in newer versions.

android_repo_complete() {
    python3 ./repo sync -c
}

# tested on 5.0.0_r1 (SDK 21)
# tested on 5.1.1_r38 (SDK 22)
android_repo_v5() {
    sync bionic
    sync build
    sync external/compiler-rt
    sync external/jemalloc
    sync external/libcxx
    sync external/libcxxabi
    sync external/libselinux
    sync external/mksh
    sync external/openssl
    sync external/pcre
    sync external/stlport
    sync prebuilts/clang/linux-x86/host/3.5
    sync system/core

    case "${ARCH}" in
        arm)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.8
        ;;
        arm64)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.8
            sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.8
        ;;
        x86_64)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.8
        ;;
    esac

    # avoid build tests
    rm bionic/linker/tests/Android.mk
    rm bionic/tests/Android.mk
    rm bionic/benchmarks/Android.mk

    # patch the linker to avoid the error
    # FATAL: kernel did not supply AT_SECURE
    sed -i -e 's/if (!kernel_supplied_AT_SECURE)/if (false)/g' bionic/linker/linker_environ.cpp
}

# tested on 6.0.1_r81 (SDK 23)
android_repo_v6() {
    sync bionic
    sync build
    sync external/compiler-rt
    sync external/elfutils
    sync external/jemalloc
    sync external/libcxx
    sync external/libcxxabi
    sync external/libselinux
    sync external/mksh
    sync external/pcre
    sync external/safe-iop
    sync external/zlib
    sync libnativehelper
    sync prebuilts/clang/linux-x86/host/3.6
    sync prebuilts/gcc/linux-x86/host/x86_64-linux-glibc2.15-4.8
    sync prebuilts/misc
    sync system/core

    case "${ARCH}" in
        arm)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
        ;;
        arm64)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
            sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
        x86_64)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
    esac

    # avoid build tests
    rm bionic/linker/tests/Android.mk
    rm bionic/tests/Android.mk
    rm bionic/benchmarks/Android.mk
    # we don't need the relocation packer, and removing
    # the unittests from it is a bit of work.
    rm bionic/tools/relocation_packer/Android.mk
}

# tested on 7.0.0_r36 (SDK 24)
# tested on 7.1.2_r39 (SDK 25, not supported)
#   API level 25, requires for Android 7.1, is not provided in NDKs
android_repo_v7() {
    sync bionic
    sync build
    sync build/kati
    sync external/boringssl
    sync external/compiler-rt
    sync external/elfutils
    sync external/jemalloc
    sync external/libcxx
    sync external/libcxxabi
    sync external/libselinux
    sync external/libunwind
    sync external/libunwind_llvm
    sync external/llvm
    sync external/mksh
    sync external/pcre
    sync external/safe-iop
    sync external/zlib
    sync prebuilts/clang/host/linux-x86
    sync prebuilts/gcc/linux-x86/host/x86_64-linux-glibc2.15-4.8
    sync prebuilts/misc
    sync prebuilts/ndk
    sync prebuilts/ninja/linux-x86
    sync system/core

    case "${ARCH}" in
        arm)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
        ;;
        arm64)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
            sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
        x86_64)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
    esac

    # avoid build tests
    rm bionic/linker/tests/Android.mk
    rm bionic/tests/Android.mk
    rm bionic/benchmarks/Android.mk
    rm prebuilts/misc/common/android-support-test/Android.mk
    # we don't need the relocation packer, and removing
    # the unittests from it is a bit of work.
    rm bionic/tools/relocation_packer/Android.mk

    remove_tests
}

# tested on 8.0.0_r51 (SDK 26)
# tested on 8.1.0_r81 (SDK 27)
android_repo_v8() {
    # need to build LLVM components, or libLLVM is disabled.
    export FORCE_BUILD_LLVM_COMPONENTS=true

    sync bionic
    sync build/blueprint
    sync build/make
    sync build/soong
    sync external/boringssl
    sync external/clang
    sync external/compiler-rt
    sync external/elfutils
    sync external/jemalloc
    sync external/libcxx
    sync external/libcxxabi
    sync external/libevent
    sync external/libunwind
    sync external/libunwind_llvm
    sync external/llvm
    sync external/lzma
    sync external/mksh
    sync external/pcre
    sync external/safe-iop
    sync external/selinux
    sync external/zlib
    sync libnativehelper
    sync prebuilts/build-tools
    sync prebuilts/clang/host/linux-x86
    sync prebuilts/gcc/linux-x86/host/x86_64-linux-glibc2.15-4.8
    sync prebuilts/go/linux-x86
    # we only need the relocation packer binary. everything else
    # interferes with the build, so we remove the makefiles below.
    sync prebuilts/misc
    sync prebuilts/ndk
    sync system/core
    sync toolchain/binutils

    case "${ARCH}" in
        arm)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
        ;;
        arm64)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
            sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
        x86_64)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
    esac

    # avoid build tests
    rm bionic/linker/tests/Android.mk
    rm bionic/tests/Android.mk
    rm bionic/tests/Android.bp
    rm bionic/benchmarks/Android.bp
    rm bionic/tests/libs/Android.bp

    # remove extra utilities
    rm system/core/libgrallocusage/Android.bp
    rm system/core/libmemtrack/Android.bp
    rm system/core/libsysutils/Android.bp
    local path=
    find prebuilts/misc/ -name 'Android.mk' | while IFS= read -r path; do
        rm "${path}"
    done

    # avoid java dependencies
    rm external/lzma/Java/Tukaani/Android.mk

    remove_tests
}

# tested on 9.0.0_r1 (SDK 28)
android_repo_v9() {
    sync art
    sync bionic
    sync build/blueprint
    sync build/make
    sync build/soong
    sync external/clang
    sync external/compiler-rt
    sync external/elfutils
    sync external/jemalloc
    sync external/libcxx
    sync external/libcxxabi
    sync external/libunwind
    sync external/libunwind_llvm
    sync external/llvm
    sync external/lzma
    sync external/mksh
    sync external/safe-iop
    sync external/valgrind
    sync external/vixl
    sync external/zlib
    sync frameworks/hardware/interfaces
    sync hardware/interfaces
    sync libnativehelper
    sync prebuilts/build-tools
    sync prebuilts/clang-tools
    sync prebuilts/clang/host/linux-x86
    sync prebuilts/gcc/linux-x86/host/x86_64-linux-glibc2.15-4.8
    sync prebuilts/go/linux-x86
    sync prebuilts/misc
    sync prebuilts/sdk
    sync system/core
    sync system/libhidl
    sync system/tools/hidl

    case "${ARCH}" in
        arm)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
        ;;
        arm64)
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
            sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
        x86_64)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
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

    remove_tests
}

# tested on 10.0.0_r47 (SDK 29)
android_repo_v10() {
    sync art
    sync bionic
    sync build/blueprint
    sync build/make
    sync build/soong
    sync external/clang
    sync external/compiler-rt
    sync external/elfutils
    sync external/golang-protobuf
    sync external/jemalloc
    sync external/jemalloc_new
    sync external/libcxx
    sync external/libcxxabi
    sync external/libunwind
    sync external/libunwind_llvm
    sync external/llvm
    sync external/lzma
    sync external/mksh
    sync external/vixl
    sync external/zlib
    sync libnativehelper
    sync prebuilts/build-tools
    sync prebuilts/clang-tools
    sync prebuilts/clang/host/linux-x86
    sync prebuilts/gcc/linux-x86/host/x86_64-linux-glibc2.17-4.8
    sync prebuilts/go/linux-x86
    sync prebuilts/ndk
    sync prebuilts/sdk
    sync prebuilts/vndk/v28
    sync system/core
    sync system/sepolicy

    case "${ARCH}" in
        arm)
            sync external/arm-optimized-routines
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
        ;;
        arm64)
            sync external/arm-optimized-routines
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
            sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
        x86_64)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
    esac

    # avoid build tests
    rm bionic/tests/Android.mk
    rm bionic/tests/Android.bp
    rm bionic/benchmarks/Android.bp
    rm bionic/tests/libs/Android.bp
    rm bionic/tests/headers/Android.bp
    rm bionic/tests/headers/posix/Android.bp

    remove_tests
}

android_repo_v11() {
    sync art
    sync bionic
    sync bootable/recovery
    sync build/blueprint
    sync build/make
    sync build/soong
    sync external/clang
    sync external/compiler-rt
    sync external/elfutils
    sync external/fmtlib
    sync external/golang-protobuf
    sync external/gwp_asan
    sync external/jemalloc
    sync external/jemalloc_new
    sync external/libcxx
    sync external/libcxxabi
    sync external/libunwind
    sync external/libunwind_llvm
    sync external/llvm
    sync external/lzma
    sync external/mksh
    sync external/scudo
    sync external/zlib
    sync prebuilts/build-tools
    sync prebuilts/clang-tools
    sync prebuilts/clang/host/linux-x86
    sync prebuilts/gcc/linux-x86/host/x86_64-linux-glibc2.17-4.8
    sync prebuilts/go/linux-x86
    sync prebuilts/sdk
    sync prebuilts/vndk/v28
    sync prebuilts/vndk/v29
    sync system/core
    sync system/sepolicy

    case "${ARCH}" in
        arm)
            sync external/arm-optimized-routines
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
        ;;
        arm64)
            sync external/arm-optimized-routines
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
            sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
        x86_64)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
    esac

    # avoid build tests
    rm bionic/tests/Android.mk
    rm bionic/tests/Android.bp
    rm bionic/benchmarks/Android.bp
    rm bionic/tests/libs/Android.bp
    rm bionic/tests/headers/Android.bp
    rm bionic/tests/headers/posix/Android.bp

    # make sure we don't build benchmarks or apex
    rm -r bionic/apex
    rm -r bionic/benchmarks/

    # libziparchive has tests in the header, remove them
    local libziparchive_h="system/core/libziparchive/include/ziparchive/zip_writer.h"
    sed -i -e 's/#include <gtest\/gtest_prod.h>//g' "${libziparchive_h}"
    sed -i -e 's/FRIEND_TEST(zipwriter, WriteToUnseekableFile);//g' "${libziparchive_h}"

    remove_tests
}

android_repo_v12() {
    # FIXME: this is a work in progress, and is unlikely to ever
    # be completed, since apex is now heavily integrated into the
    # build system. `external/mksh` and `system/core/toolbox` build,
    # however, `bionic`, the most import module, does not.
    #
    # the error messages are of the following:
    #   internal error: panic in GenerateBuildActions for module "com.android.example.apex" variant "android_common_com.android.example.apex_image"
    # fixing this requires either a comprehensive removal of APEX from the build
    # or adding numerous APEX dependencies, which defeats the purpose of a
    # minimal bionic build.
    sync art
    sync bionic
    sync build/blueprint
    sync build/make
    sync build/soong
    sync external/apache-xml
    sync external/bouncycastle
    sync external/clang
    sync external/compiler-rt
    sync external/conscrypt
    sync external/elfutils
    sync external/fmtlib
    sync external/golang-protobuf
    sync external/gwp_asan
    sync external/icu
    sync external/jemalloc
    sync external/jemalloc_new
    sync external/libcxx
    sync external/libcxxabi
    sync external/libunwind
    sync external/libunwind_llvm
    sync external/llvm
    sync external/lzma
    sync external/mksh
    sync external/okhttp
    sync external/scudo
    sync external/starlark-go
    sync external/zlib
    sync libcore
    sync prebuilts/build-tools
    sync prebuilts/clang-tools
    sync prebuilts/clang/host/linux-x86
    sync prebuilts/gcc/linux-x86/host/x86_64-linux-glibc2.17-4.8
    sync prebuilts/go/linux-x86
    sync prebuilts/sdk
    sync prebuilts/vndk/v28
    sync prebuilts/vndk/v29
    sync prebuilts/vndk/v30
    sync system/core
    sync system/libbase
    sync system/linkerconfig
    sync system/logging
    sync system/sepolicy
    sync system/tools/xsdc
    sync tools/metalava
    # these tools also seem to be required, since apex is now tightly
    # coupled with the bionic build. unfortunately, we want to avoid
    # building apex at all costs.
    #sync system/apex
    #sync system/tools/aidl

    case "${ARCH}" in
        arm)
            sync external/arm-optimized-routines
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
        ;;
        arm64)
            sync external/arm-optimized-routines
            sync prebuilts/gcc/linux-x86/arm/arm-linux-androideabi-4.9
            sync prebuilts/gcc/linux-x86/aarch64/aarch64-linux-android-4.9
        ;;
        x86)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
        x86_64)
            sync prebuilts/gcc/linux-x86/x86/x86_64-linux-android-4.9
        ;;
    esac

    # avoid build tests
    rm bionic/tests/Android.mk
    rm bionic/tests/Android.bp
    rm bionic/benchmarks/Android.bp
    rm bionic/tests/libs/Android.bp
    rm bionic/tests/headers/Android.bp
    rm bionic/tests/headers/posix/Android.bp

    # make sure we don't build benchmarks or apex
    rm -r bionic/apex
    rm -r bionic/benchmarks/
    rm -r bionic/tests/
    rm -r system/linkerconfig/testmodules

    remove_tests
}

remove_tests() {
    install_packages python3-pip

    local version=
    version=$(python3 -c 'import sys
major = sys.version_info.major
minor = sys.version_info.minor
print(f"{major}.{minor}")')
    set +u
    export PYTHONPATH="${PYTHON_TMPDIR}/lib/python${version}/site-packages/:${PYTHONPATH}"
    set -u
    mkdir -p "${PYTHON_TMPDIR}"
    python3 -m pip install sly==0.4.0 --prefix "${PYTHON_TMPDIR}"
    python3 -m pip install google-re2==1.0 --prefix "${PYTHON_TMPDIR}"

    python3 "${PYTHON_TMPDIR}/scripts/build-system.py" \
        --remove-tests \
        --verbose
}

sync() {
    python3 ./repo sync -c --no-clone-bundle "${1}"
}

main "${@}"
