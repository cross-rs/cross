FROM ubuntu:16.04

COPY freebsd.sh /
RUN bash /freebsd.sh x86_64

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j BSD-x86_64 x86_64-unknown-freebsd10-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_X86_64_UNKNOWN_FREEBSD_LINKER=x86_64-unknown-freebsd10-gcc \
    CC_x86_64_unknown_freebsd=x86_64-unknown-freebsd10-gcc \
    OPENSSL_DIR=/openssl
