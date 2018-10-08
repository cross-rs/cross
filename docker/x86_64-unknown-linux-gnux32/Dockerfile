FROM ubuntu:14.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    g++-multilib \
    libc6-dev \
    make \
    pkg-config

COPY xargo.sh /
RUN bash /xargo.sh

COPY cmake.sh /
RUN apt-get purge --auto-remove -y cmake && \
    bash /cmake.sh 3.5.1

COPY openssl.sh /
RUN bash /openssl.sh linux-x32

ENV OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
