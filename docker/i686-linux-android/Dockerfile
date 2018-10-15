FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

COPY xargo.sh /
RUN bash /xargo.sh

COPY android-ndk.sh /
RUN bash /android-ndk.sh x86 21
ENV PATH=$PATH:/android-ndk/bin

COPY android-system.sh /
RUN bash /android-system.sh x86

# We could supposedly directly run i686 binaries like we do for x86_64, but
# doing so generates an assertion failure:
#   ... assertion failed: signal(libc::SIGPIPE, libc::SIG_IGN) != libc::SIG_ERR
#   ... src/libstd/sys/unix/mod.rs
#   fatal runtime error: failed to initiate panic, error 5
#
# Running with qemu works as expected
COPY qemu.sh /
RUN bash /qemu.sh i386 android

# Build with no-asm to make openssl linked binaries position-independent (PIE)
COPY openssl.sh /
RUN bash /openssl.sh android-x86 i686-linux-android- no-asm

RUN cp /android-ndk/sysroot/usr/lib/libz.so /system/lib/

# Libz is distributed in the android ndk, but for some unknown reason it is not
# found in the build process of some crates, so we explicit set the DEP_Z_ROOT
ENV CARGO_TARGET_I686_LINUX_ANDROID_LINKER=i686-linux-android-gcc \
    CARGO_TARGET_I686_LINUX_ANDROID_RUNNER="qemu-i386 -cpu n270" \
    CC_i686_linux_android=i686-linux-android-gcc \
    CXX_i686_linux_android=i686-linux-android-g++ \
    DEP_Z_INCLUDE=/android-ndk/sysroot/usr/include/ \
    LIBZ_SYS_STATIC=1 \
    OPENSSL_STATIC=1 \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1 \
    HOME=/tmp/ \
    TMPDIR=/tmp/ \
    ANDROID_DATA=/ \
    ANDROID_DNS_MODE=local \
    ANDROID_ROOT=/system
