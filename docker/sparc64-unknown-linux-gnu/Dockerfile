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
    g++-sparc64-linux-gnu \
    libc6-dev-sparc64-cross && \
    bash /openssl.sh linux64-sparcv9 sparc64-linux-gnu- && \
    bash /qemu.sh sparc64

ENV CARGO_TARGET_SPARC64_UNKNOWN_LINUX_GNU_LINKER=sparc64-linux-gnu-gcc \
    CC_sparc64_unknown_linux_gnu=sparc64-linux-gnu-gcc \
    CXX_sparc64_unknown_linux_gnu=sparc64-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/sparc64-linux-gnu \
    RUST_TEST_THREADS=1
