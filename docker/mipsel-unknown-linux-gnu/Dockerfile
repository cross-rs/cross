FROM ubuntu:16.04

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
RUN apt-get install -y --no-install-recommends \
    g++-mipsel-linux-gnu \
    libc6-dev-mipsel-cross && \
    bash /openssl.sh linux-mips32 mipsel-linux-gnu- && \
    bash /qemu.sh mipsel

ENV CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_GNU_LINKER=mipsel-linux-gnu-gcc \
    CC_mipsel_unknown_linux_gnu=mipsel-linux-gnu-gcc \
    CXX_mipsel_unknown_linux_gnu=mipsel-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/mipsel-linux-gnu \
    RUST_TEST_THREADS=1
