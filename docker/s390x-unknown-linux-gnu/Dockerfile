FROM ubuntu:16.04

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 s390x

RUN apt-get install -y --no-install-recommends \
    gcc-s390x-linux-gnu \
    libc6-dev-s390x-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux64-s390x s390x-linux-gnu-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    g++-s390x-linux-gnu \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_LINKER=s390x-linux-gnu-gcc \
    CC_s390x_unknown_linux_gnu=s390x-linux-gnu-gcc \
    CXX_s390x_unknown_linux_gnu=s390x-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/s390x-linux-gnu \
    RUST_TEST_THREADS=1
