FROM ubuntu:16.04

COPY freebsd.sh /
RUN bash /freebsd.sh i686

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j BSD-x86-elf i686-unknown-freebsd10-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_I686_UNKNOWN_FREEBSD_LINKER=i686-unknown-freebsd10-gcc \
    CC_i686_unknown_freebsd=i686-unknown-freebsd10-gcc \
    CXX_i686_unknown_freebsd=i686-unknown-freebsd10-g++ \
    OPENSSL_DIR=/openssl
