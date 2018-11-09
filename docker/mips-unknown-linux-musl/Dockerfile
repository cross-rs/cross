FROM ubuntu:18.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config \
    g++ \
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

COPY qemu.sh /
RUN bash /qemu.sh mips

COPY musl.sh /
RUN bash /musl.sh \
    TARGET=mips-linux-muslsf \
    "COMMON_CONFIG += -with-arch=mips32r2"

COPY openssl.sh /
RUN bash /openssl.sh linux-mips32 mips-linux-muslsf-

# Allows qemu run dynamic linked binaries
RUN ln -sf \
    /usr/local/mips-linux-muslsf/lib/libc.so \
    /usr/local/mips-linux-muslsf/lib/ld-musl-mips-sf.so.1
ENV QEMU_LD_PREFIX=/usr/local/mips-linux-muslsf

ENV CARGO_TARGET_MIPS_UNKNOWN_LINUX_MUSL_LINKER=mips-linux-muslsf-gcc \
    CARGO_TARGET_MIPS_UNKNOWN_LINUX_MUSL_RUNNER=qemu-mips \
    CC_mips_unknown_linux_musl=mips-linux-muslsf-gcc \
    CXX_mips_unknown_linux_musl=mips-linux-muslsf-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1 \
    QEMU_LD_PREFIX=/usr/local/mips-linux-muslsf
