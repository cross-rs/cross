FROM ubuntu:18.04

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
RUN bash /solaris.sh sparcv9 && \
    bash /openssl.sh solaris64-sparcv9-gcc sparcv9-sun-solaris2.10-

ENV CARGO_TARGET_SPARCV9_SUN_SOLARIS_LINKER=sparcv9-sun-solaris2.10-gcc \
    CC_sparcv9_sun_solaris=sparcv9-sun-solaris2.10-gcc \
    CXX_sparcv9_sun_solaris=sparcv9-sun-solaris2.10-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
