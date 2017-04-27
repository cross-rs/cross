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

COPY qemu.sh /
RUN bash /qemu.sh aarch64 android

COPY android-ndk.sh /
RUN bash /android-ndk.sh arm64 21
ENV PATH=$PATH:/android-ndk/bin

COPY openssl.sh /
RUN bash /openssl.sh linux-generic64 aarch64-linux-android- -mandroid -fomit-frame-pointer

COPY android-system.sh /
RUN bash /android-system.sh arm64

# Libz is distributed in the android ndk, but for some unknown reason it is not
# found in the build process of some crates, so we explicit set the DEP_Z_ROOT
ENV CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android-gcc \
    CC_aarch64_linux_android=aarch64-linux-android-gcc \
    CXX_aarch64_linux_android=aarch64-linux-android-g++ \
    DEP_Z_ROOT=/android-ndk/sysroot/usr/ \
    OPENSSL_STATIC=1 \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    HOME=/tmp/ \
    TMPDIR=/tmp/ \
    ANDROID_ROOT=/system \
    ANDROID_DATA=/
