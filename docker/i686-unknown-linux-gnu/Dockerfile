FROM ubuntu:12.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

COPY cmake.sh /
RUN apt-get purge --auto-remove -y cmake && \
    bash /cmake.sh 2.8.11

COPY openssl.sh /
RUN apt-get install -y --no-install-recommends \
    g++-multilib && \
    bash /openssl.sh linux-elf "" -m32

RUN apt-get install -y --no-install-recommends \
    curl && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

ENV OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
