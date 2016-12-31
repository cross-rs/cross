FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    gcc

COPY musl.sh /
RUN bash /musl.sh 1.1.15
COPY musl-gcc.x86_64-unknown-linux-musl /usr/local/bin/musl-gcc
COPY musl-gcc.specs.x86_64-unknown-linux-musl /usr/local/lib/musl-gcc.specs

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-x86_64 musl- -static

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    libc6-dev \
    make \
    pkg-config

ENV CC_i686_unknown_linux_musl=musl-gcc \
    OPENSSL_DIR=/openssl
