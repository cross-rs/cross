FROM ubuntu:18.04

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
RUN bash /android-ndk.sh x86_64 21
ENV PATH=$PATH:/android-ndk/bin

COPY openssl.sh /
RUN bash /openssl.sh linux-x86_64 x86_64-linux-android- -mandroid -fomit-frame-pointer

COPY android-system.sh /
RUN bash /android-system.sh x86_64

# Libz is distributed in the android ndk, but for some unknown reason it is not
# found in the build process of some crates, so we explicit set the DEP_Z_ROOT
ENV CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=x86_64-linux-android-gcc \
    CC_x86_64_linux_android=x86_64-linux-android-gcc \
    CXX_x86_64_linux_android=x86_64-linux-android-g++ \
    DEP_Z_ROOT=/android-ndk/sysroot/usr/ \
    OPENSSL_STATIC=1 \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    HOME=/tmp/ \
    TMPDIR=/tmp/ \
    ANDROID_ROOT=/system \
    ANDROID_DATA=/
