FROM ubuntu:12.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    curl \
    gcc \
    libc6-dev \
    make \
    pkg-config && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

COPY cmake.sh /
RUN apt-get purge --auto-remove -y cmake && \
    bash /cmake.sh 2.8.11

COPY openssl.sh /
RUN apt-get install -y --no-install-recommends \
    g++ \
    zlib1g-dev && \
    bash /openssl.sh 1.0.2j linux-x86_64

ENV OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
