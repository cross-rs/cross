FROM ubuntu:14.04

COPY qemu.sh /
RUN bash /qemu.sh 2.7.1 ppc

RUN apt-get install -y --no-install-recommends \
    gcc-powerpc-linux-gnu \
    libc6-dev-powerpc-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-ppc powerpc-linux-gnu-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    g++-powerpc-linux-gnu \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_POWERPC_UNKNOWN_LINUX_GNU_LINKER=powerpc-linux-gnu-gcc \
    CC_powerpc_unknown_linux_gnu=powerpc-linux-gnu-gcc \
    CXX_powerpc_unknown_linux_gnu=powerpc-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/powerpc-linux-gnu \
    RUST_TEST_THREADS=1
