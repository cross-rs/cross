FROM debian:jessie

RUN apt-get update && apt-get install -y software-properties-common wget curl

RUN add-apt-repository "deb http://emdebian.org/tools/debian/ jessie main" && \
    curl http://emdebian.org/tools/debian/emdebian-toolchain-archive.key | apt-key add - && \
    dpkg --add-architecture armel && \
    apt-get update && \
    apt-get install -y crossbuild-essential-armel

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

COPY openssl.sh qemu.sh /
RUN bash /openssl.sh linux-armv4 arm-linux-gnueabi- && \
    bash /qemu.sh arm

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABI_OSATOMIC_LINKER=arm-linux-gnueabi-gcc \
    HOST=x86_64-unknown-linux-gnu \
    CC_armv5te_unknown_linux_gnueabi_osatomic=arm-linux-gnueabi-gcc \
    CXX_armv5te_unknown_linux_gnueabi_osatomic=arm-linux-gnueabi-g++ \
    CFLAGS_armv5te_unknown_linux_gnueabi_osatomic="-Wall -Os -fPIC -D__arm__ -mfloat-abi=soft" \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/arm-linux-gnueabi \
    RUST_TEST_THREADS=1

