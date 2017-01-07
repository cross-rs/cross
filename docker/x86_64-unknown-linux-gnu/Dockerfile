FROM ubuntu:12.04

COPY cmake.sh /
RUN bash /cmake.sh 2.8.11

RUN apt-get install -y --no-install-recommends \
    gcc \
    libc6-dev

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-x86_64

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    g++ \
    make \
    pkg-config \
    zlib1g-dev

ENV OPENSSL_DIR=/openssl
