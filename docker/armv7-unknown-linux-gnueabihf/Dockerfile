FROM ubuntu:14.04

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

COPY cmake.sh /
RUN apt-get purge --auto-remove -y cmake && \
    bash /cmake.sh 2.8.11

COPY openssl.sh qemu.sh /
RUN apt-get install -y --no-install-recommends \
    g++-arm-linux-gnueabihf \
    libc6-dev-armhf-cross && \
    bash /openssl.sh linux-armv4 arm-linux-gnueabihf- && \
    bash /qemu.sh arm

ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUNNER=qemu-arm \
    CC_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    CXX_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/arm-linux-gnueabihf \
    RUST_TEST_THREADS=1
