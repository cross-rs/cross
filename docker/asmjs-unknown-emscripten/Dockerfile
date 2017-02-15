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

ENV NODE=/usr/bin/nodejs
