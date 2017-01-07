FROM ubuntu:14.04

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 arm

RUN apt-get install -y --no-install-recommends \
    gcc-arm-linux-gnueabi \
    libc6-dev-armel-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-armv4 arm-linux-gnueabi-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    g++-arm-linux-gnueabi \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABI_LINKER=arm-linux-gnueabi-gcc \
    CC_arm_unknown_linux_gnueabi=arm-linux-gnueabi-gcc \
    CXX_arm_unknown_linux_gnueabi=arm-linux-gnueabi-g++ \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/arm-linux-gnueabi \
    RUST_TEST_THREADS=1
