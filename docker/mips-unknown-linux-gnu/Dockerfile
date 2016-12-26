FROM ubuntu:16.04

COPY override.sh /
RUN bash /override.sh curl-sys 0.3.6

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 mips

RUN apt-get install -y --no-install-recommends \
    gcc-mips-linux-gnu \
    libc6-dev-mips-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-mips32 mips-linux-gnu-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_MIPS_UNKNOWN_LINUX_GNU_LINKER=mips-linux-gnu-gcc \
    CC_mips_unknown_linux_gnu=mips-linux-gnu-gcc \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/mips-linux-gnu \
    RUST_TEST_THREADS=1
