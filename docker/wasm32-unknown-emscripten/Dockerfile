FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

COPY emscripten.sh /
RUN bash /emscripten.sh 1.37.2

RUN apt-get install --no-install-recommends -y \
    nodejs \
    python

COPY binaryen.sh /
RUN bash /binaryen.sh version_27

ENV BINARYEN=/binaryen \
    NODE=/usr/bin/nodejs
