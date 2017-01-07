FROM ubuntu:12.04

COPY cmake.sh /
RUN bash /cmake.sh 2.8.11

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 arm

RUN apt-get install -y --no-install-recommends \
    gcc-arm-linux-gnueabihf \
    libc6-dev-armhf-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-armv4 arm-linux-gnueabihf-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    g++-arm-linux-gnueabihf \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CC_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    CXX_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++ \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/arm-linux-gnueabihf \
    RUST_TEST_THREADS=1
