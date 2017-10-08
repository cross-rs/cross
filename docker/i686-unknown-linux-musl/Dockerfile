FROM i386/ubuntu:16.04

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

COPY musl.sh /
RUN apt-get install -y --no-install-recommends \
    gcc-multilib && \
    bash /musl.sh 1.1.15 i386 -m32

ENV CC_i686_unknown_linux_musl=musl-gcc

COPY openssl.sh /
RUN apt-get install -y --no-install-recommends \
    g++-multilib && \
    bash /openssl.sh linux-elf musl- -m32

ENV OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
