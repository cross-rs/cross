FROM ubuntu:16.04

COPY qemu.sh /
RUN bash /qemu.sh 2.7.1 ppc64

RUN apt-get install -y --no-install-recommends \
    gcc-powerpc64-linux-gnu \
    libc6-dev-ppc64-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-ppc64 powerpc64-linux-gnu-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_POWERPC64_UNKNOWN_LINUX_GNU_LINKER=powerpc64-linux-gnu-gcc \
    CC_powerpc64_unknown_linux_gnu=powerpc64-linux-gnu-gcc \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/powerpc64-linux-gnu \
    RUST_TEST_THREADS=1
