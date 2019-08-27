FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config \
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

RUN apt-get install -y --no-install-recommends \
    g++-powerpc64-linux-gnu \
    libc6-dev-ppc64-cross

COPY openssl.sh /
RUN bash /openssl.sh linux-ppc64 powerpc64-linux-gnu-

COPY qemu.sh /
RUN bash /qemu.sh ppc64 linux softmmu

COPY dropbear.sh /
RUN bash /dropbear.sh

COPY linux-image.sh /
RUN bash /linux-image.sh powerpc64

COPY linux-runner /

ENV CARGO_TARGET_POWERPC64_UNKNOWN_LINUX_GNU_LINKER=powerpc64-linux-gnu-gcc \
    CARGO_TARGET_POWERPC64_UNKNOWN_LINUX_GNU_RUNNER="/linux-runner powerpc64" \
    CC_powerpc64_unknown_linux_gnu=powerpc64-linux-gnu-gcc \
    CXX_powerpc64_unknown_linux_gnu=powerpc64-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/powerpc64-linux-gnu \
    RUST_TEST_THREADS=1
