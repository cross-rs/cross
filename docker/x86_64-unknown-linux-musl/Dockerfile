FROM ubuntu:16.04

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
    sh -s -- --git japaric/xargo --tag v0.3.4 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

COPY musl.sh openssl.sh /
COPY musl-gcc.x86_64-unknown-linux-musl /usr/local/bin/musl-gcc
COPY musl-gcc.specs.x86_64-unknown-linux-musl /usr/local/lib/musl-gcc.specs
RUN bash /musl.sh 1.1.15 && \
    bash /openssl.sh 1.0.2j linux-x86_64 musl- -static

ENV CC_x86_64_unknown_linux_musl=musl-gcc \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
