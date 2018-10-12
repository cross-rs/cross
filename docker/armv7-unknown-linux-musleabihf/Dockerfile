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

COPY qemu.sh /
RUN bash /qemu.sh arm

COPY musl.sh /
RUN bash /musl.sh \
    TARGET=arm-linux-musleabihf \
    "COMMON_CONFIG += --with-arch=armv7-a \
                      --with-fpu=neon \
                      --with-float=hard \
                      --with-mode=thumb"

COPY openssl.sh /
RUN bash /openssl.sh linux-armv4 arm-linux-musleabihf-

# Allows qemu run dynamic linked binaries
RUN ln -sf \
    /usr/local/arm-linux-musleabihf/lib/libc.so \
    /usr/local/arm-linux-musleabihf/lib/ld-musl-armhf.so.1
ENV QEMU_LD_PREFIX=/usr/local/arm-linux-musleabihd

ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER=arm-linux-musleabihf-gcc \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_RUNNER=qemu-arm \
    CC_armv7_unknown_linux_musleabihf=arm-linux-musleabihf-gcc \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1
