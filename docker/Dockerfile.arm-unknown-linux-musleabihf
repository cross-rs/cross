FROM ubuntu:18.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config \
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
    TARGET=arm-linux-musleabihf \
    "COMMON_CONFIG += --with-arch=armv6 \
                      --with-fpu=vfp \
                      --with-float=hard \
                      --with-mode=arm"

COPY openssl.sh /
RUN bash /openssl.sh linux-armv4 arm-linux-musleabihf-

# Allows qemu run dynamic linked binaries
RUN ln -sf \
    /usr/local/arm-linux-musleabihf/lib/libc.so \
    /usr/local/arm-linux-musleabihf/lib/ld-musl-armhf.so.1
ENV QEMU_LD_PREFIX=/usr/local/arm-linux-musleabihf

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABIHF_LINKER=arm-linux-musleabihf-gcc \
    CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABIHF_RUNNER=qemu-arm \
    CC_arm_unknown_linux_musleabihf=arm-linux-musleabihf-gcc \
    CXX_arm_unknown_linux_musleabihf=arm-linux-musleabihf-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1
