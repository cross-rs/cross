FROM ubuntu:16.04

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

COPY cmake.sh /
RUN apt-get purge --auto-remove -y cmake && \
    bash /cmake.sh 3.5.1

RUN mkdir /usr/arm-linux-gnueabihf && \
    apt-get install -y --no-install-recommends curl && \
    cd /usr/arm-linux-gnueabihf && \
    curl -L https://toolchains.bootlin.com/downloads/releases/toolchains/armv6-eabihf/tarballs/armv6-eabihf--glibc--stable-2018.11-1.tar.bz2 | \
    tar --strip-components 1 -xj && \
    apt-get purge --auto-remove -y curl

COPY openssl.sh qemu.sh /

RUN bash /qemu.sh arm
ENV PATH /usr/arm-linux-gnueabihf/bin:$PATH

RUN apt-get install -y --no-install-recommends \
    libc6-dev-armhf-cross && \
    bash /openssl.sh linux-armv4 arm-linux-

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gcc \
    CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_RUNNER=qemu-arm \
    CC_arm_unknown_linux_gnueabihf=arm-linux-gcc \
    CXX_arm_unknown_linux_gnueabihf=arm-linux-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/arm-linux-gnueabihf \
    LD_LIBRARY_PATH=/usr/arm-linux-gnueabihf/lib:/usr/arm-linux-gnueabihf/arm-buildroot-linux-gnueabihf/lib \
    RUST_TEST_THREADS=1
