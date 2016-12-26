FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    musl-tools && \
    ln -s /usr/bin/ar /usr/bin/musl-ar

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-x86_64 musl-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    libc6-dev \
    make \
    pkg-config

ENV OPENSSL_DIR=/openssl
