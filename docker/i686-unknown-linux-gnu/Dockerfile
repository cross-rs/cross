FROM ubuntu:12.04

COPY cmake.sh /
RUN bash /cmake.sh 2.8.11

RUN apt-get install -y --no-install-recommends \
    gcc-multilib

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-elf "" -m32

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    make \
    pkg-config

ENV OPENSSL_DIR=/openssl
