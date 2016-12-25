FROM ubuntu:16.04

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 mipsel

RUN apt-get install -y --no-install-recommends \
    gcc-mipsel-linux-gnu \
    libc6-dev-mipsel-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-mips32 mipsel-linux-gnu-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_GNU_LINKER=mipsel-linux-gnu-gcc \
    CC_mipsel_unknown_linux_gnu=mipsel-linux-gnu-gcc \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/mipsel-linux-gnu \
    RUST_TEST_THREADS=1
