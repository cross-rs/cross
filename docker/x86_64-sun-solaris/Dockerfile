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

COPY solaris.sh openssl.sh /
RUN bash /solaris.sh x86_64 && \
    bash /openssl.sh solaris64-x86_64-gcc x86_64-sun-solaris2.11-

ENV CARGO_TARGET_X86_64_SUN_SOLARIS_LINKER=x86_64-sun-solaris2.11-gcc \
    CC_x86_64_sun_solaris=x86_64-sun-solaris2.11-gcc \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
