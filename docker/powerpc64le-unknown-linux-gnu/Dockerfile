FROM ubuntu:16.04

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

RUN apt-get install -y --no-install-recommends \
    g++-powerpc64le-linux-gnu \
    libc6-dev-ppc64el-cross

COPY openssl.sh /
RUN bash /openssl.sh linux-ppc64le powerpc64le-linux-gnu-

COPY qemu.sh /
RUN bash /qemu.sh ppc64le linux softmmu

COPY dropbear.sh /
RUN bash /dropbear.sh

COPY linux-image.sh /
RUN bash /linux-image.sh powerpc64le

COPY linux-runner /

ENV CARGO_TARGET_POWERPC64LE_UNKNOWN_LINUX_GNU_LINKER=powerpc64le-linux-gnu-gcc \
    CARGO_TARGET_POWERPC64LE_UNKNOWN_LINUX_GNU_RUNNER="/linux-runner powerpc64le" \
    CC_powerpc64le_unknown_linux_gnu=powerpc64le-linux-gnu-gcc \
    CXX_powerpc64le_unknown_linux_gnu=powerpc64le-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/powerpc64le-linux-gnu \
    RUST_TEST_THREADS=1
