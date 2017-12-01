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

RUN apt-get install -y --no-install-recommends \
    g++-mips-linux-gnu \
    libc6-dev-mips-cross

COPY openssl.sh /
RUN bash /openssl.sh linux-mips32 mips-linux-gnu-

COPY qemu.sh /
RUN bash /qemu.sh mips linux softmmu

COPY dropbear.sh /
RUN bash /dropbear.sh

COPY linux-image.sh /
RUN bash /linux-image.sh mips

COPY linux-runner /

ENV CARGO_TARGET_MIPS_UNKNOWN_LINUX_GNU_LINKER=mips-linux-gnu-gcc \
    CARGO_TARGET_MIPS_UNKNOWN_LINUX_GNU_RUNNER="/linux-runner mips" \
    CC_mips_unknown_linux_gnu=mips-linux-gnu-gcc \
    CXX_mips_unknown_linux_gnu=mips-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/mips-linux-gnu \
    RUST_TEST_THREADS=1
