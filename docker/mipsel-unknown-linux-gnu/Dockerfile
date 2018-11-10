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

RUN apt-get install -y --no-install-recommends \
    g++-mipsel-linux-gnu \
    libc6-dev-mipsel-cross

COPY openssl.sh /
RUN bash /openssl.sh linux-mips32 mipsel-linux-gnu-

COPY qemu.sh /
RUN bash /qemu.sh mipsel linux softmmu

COPY dropbear.sh /
RUN bash /dropbear.sh

COPY linux-image.sh /
RUN bash /linux-image.sh mipsel

COPY linux-runner /

ENV CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_GNU_LINKER=mipsel-linux-gnu-gcc \
    CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_GNU_RUNNER="/linux-runner mipsel" \
    CC_mipsel_unknown_linux_gnu=mipsel-linux-gnu-gcc \
    CXX_mipsel_unknown_linux_gnu=mipsel-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/mipsel-linux-gnu \
    RUST_TEST_THREADS=1
