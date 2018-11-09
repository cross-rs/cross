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

COPY musl.sh /
RUN bash /musl.sh TARGET=i586-linux-musl

COPY openssl.sh /
RUN bash /openssl.sh linux-elf i586-linux-musl-

ENV CARGO_TARGET_I586_UNKNOWN_LINUX_MUSL_LINKER=i586-linux-musl-gcc \
    CC_i586_unknown_linux_musl=i586-linux-musl-gcc \
    CXX_i586_unknown_linux_musl=i586-linux-musl-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
