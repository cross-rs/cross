FROM ubuntu:18.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config \
    g++ \
    git \
    automake \
    libtool \
    m4 \
    autoconf \
    make \
    file \
    binutils

COPY xargo.sh /
RUN bash /xargo.sh

COPY qemu.sh /
RUN bash /qemu.sh arm

COPY musl.sh /
RUN bash /musl.sh \
    TARGET=arm-linux-musleabi \
    "COMMON_CONFIG += --with-arch=armv6 \
                      --with-float=soft \
                      --with-mode=arm"

COPY openssl.sh /
RUN bash /openssl.sh linux-armv4 arm-linux-musleabi-

# Allows qemu run dynamic linked binaries
RUN ln -sf \
    /usr/local/arm-linux-musleabi/lib/libc.so \
    /usr/local/arm-linux-musleabi/lib/ld-musl-arm.so.1
ENV QEMU_LD_PREFIX=/usr/local/arm-linux-musleabi

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABI_LINKER=arm-linux-musleabi-gcc \
    CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABI_RUNNER=qemu-arm \
    CC_arm_unknown_linux_musleabi=arm-linux-musleabi-gcc \
    CXX_arm_unknown_linux_musleabi=arm-linux-musleabi-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1
