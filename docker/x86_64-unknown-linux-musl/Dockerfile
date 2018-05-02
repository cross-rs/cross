FROM ubuntu:18.04

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

COPY musl.sh openssl.sh /
COPY musl-gcc.x86_64-unknown-linux-musl /usr/local/bin/musl-gcc
COPY musl-gcc.specs.x86_64-unknown-linux-musl /usr/local/lib/musl-gcc.specs
RUN bash /musl.sh 1.1.15 && \
    bash /openssl.sh linux-x86_64 musl- -static

ENV CC_x86_64_unknown_linux_musl=musl-gcc \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
