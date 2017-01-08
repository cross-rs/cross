FROM ubuntu:16.04

COPY dragonfly.sh /
RUN bash /dragonfly.sh

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j BSD-x86_64 x86_64-unknown-dragonfly-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_X86_64_UNKNOWN_DRAGONFLY_LINKER=x86_64-unknown-dragonfly-gcc \
    CC_x86_64_unknown_dragonfly=x86_64-unknown-dragonfly-gcc \
    OPENSSL_DIR=/openssl
