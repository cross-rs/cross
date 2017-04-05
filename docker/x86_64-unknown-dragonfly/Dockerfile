FROM ubuntu:16.04

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

COPY dragonfly.sh openssl.sh /
RUN bash /dragonfly.sh && \
    bash /openssl.sh BSD-x86_64 x86_64-unknown-dragonfly-

ENV CARGO_TARGET_X86_64_UNKNOWN_DRAGONFLY_LINKER=x86_64-unknown-dragonfly-gcc \
    CC_x86_64_unknown_dragonfly=x86_64-unknown-dragonfly-gcc \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
