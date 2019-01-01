FROM ubuntu:14.04

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

RUN git clone --depth 1 https://github.com/raspberrypi/tools.git /pi-tools && \
    mv /pi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64 /usr/arm-linux-gnueabihf && \
    mv /pi-tools/arm-bcm2708/arm-bcm2708hardfp-linux-gnueabi/arm-bcm2708hardfp-linux-gnueabi/sysroot /usr/arm-linux-gnueabihf/sysroot && \
    rm -r /pi-tools

ENV PATH /usr/arm-linux-gnueabihf/bin:$PATH

COPY openssl.sh qemu.sh /
RUN apt-get install -y --no-install-recommends \
    libc6-dev-armhf-cross && \
    bash /openssl.sh linux-armv4 arm-linux-gnueabihf- && \
    bash /qemu.sh arm

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_RUNNER=qemu-arm \
    CC_arm_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    CXX_arm_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/arm-linux-gnueabihf \
    LD_LIBRARY_PATH=/usr/arm-linux-gnueabihf/lib:/usr/arm-linux-gnueabihf/sysroot/lib \
    RUST_TEST_THREADS=1
