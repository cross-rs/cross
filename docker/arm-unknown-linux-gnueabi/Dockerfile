FROM ubuntu:14.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

COPY openssl.sh qemu.sh /
RUN apt-get install -y --no-install-recommends \
    g++-arm-linux-gnueabi \
    libc6-dev-armel-cross && \
    bash /openssl.sh linux-armv4 arm-linux-gnueabi- && \
    bash /qemu.sh 2.8.0 arm

RUN apt-get install -y --no-install-recommends \
    curl && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABI_LINKER=arm-linux-gnueabi-gcc \
    CC_arm_unknown_linux_gnueabi=arm-linux-gnueabi-gcc \
    CXX_arm_unknown_linux_gnueabi=arm-linux-gnueabi-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/arm-linux-gnueabi \
    RUST_TEST_THREADS=1
