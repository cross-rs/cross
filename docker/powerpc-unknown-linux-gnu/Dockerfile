FROM ubuntu:14.04

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
    g++-powerpc-linux-gnu \
    libc6-dev-powerpc-cross && \
    bash /openssl.sh linux-ppc powerpc-linux-gnu- && \
    bash /qemu.sh 2.7.1 ppc

ENV CARGO_TARGET_POWERPC_UNKNOWN_LINUX_GNU_LINKER=powerpc-linux-gnu-gcc \
    CC_powerpc_unknown_linux_gnu=powerpc-linux-gnu-gcc \
    CXX_powerpc_unknown_linux_gnu=powerpc-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/powerpc-linux-gnu \
    RUST_TEST_THREADS=1
