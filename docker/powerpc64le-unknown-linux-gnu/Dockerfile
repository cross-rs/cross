FROM ubuntu:16.04

COPY qemu.sh /
RUN bash /qemu.sh 2.7.1 ppc64le

RUN apt-get install -y --no-install-recommends \
    gcc-powerpc64le-linux-gnu \
    libc6-dev-ppc64el-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-ppc64le powerpc64le-linux-gnu-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_POWERPC64LE_UNKNOWN_LINUX_GNU_LINKER=powerpc64le-linux-gnu-gcc \
    CC_powerpc64le_unknown_linux_gnu=powerpc64le-linux-gnu-gcc \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/powerpc64le-linux-gnu \
    RUST_TEST_THREADS=1
