FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    gcc-multilib

COPY musl.sh /
RUN bash /musl.sh 1.1.15 i386 -m32

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    make \
    pkg-config

ENV CC_i686_unknown_linux_musl=musl-gcc
