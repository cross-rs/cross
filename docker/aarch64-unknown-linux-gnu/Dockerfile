FROM ubuntu:14.04

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 aarch64

RUN apt-get install -y --no-install-recommends \
    gcc-aarch64-linux-gnu \
    libc6-dev-arm64-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-aarch64 aarch64-linux-gnu-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/aarch64-linux-gnu \
    RUST_TEST_THREADS=1
