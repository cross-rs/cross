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
    g++-s390x-linux-gnu \
    libc6-dev-s390x-cross

COPY openssl.sh /
RUN bash /openssl.sh linux64-s390x s390x-linux-gnu-

COPY qemu.sh /
RUN bash /qemu.sh s390x linux softmmu

COPY dropbear.sh /
RUN bash /dropbear.sh

COPY linux-image.sh /
RUN bash /linux-image.sh s390x

COPY linux-runner /

ENV CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_LINKER=s390x-linux-gnu-gcc \
    CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_RUNNER="/linux-runner s390x" \
    CC_s390x_unknown_linux_gnu=s390x-linux-gnu-gcc \
    CXX_s390x_unknown_linux_gnu=s390x-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/s390x-linux-gnu \
    RUST_TEST_THREADS=1
