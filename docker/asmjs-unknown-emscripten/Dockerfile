FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config \
    g++ \
    git \
    automake \
    libtool \
    m4 \
    autoconf \
    make \
    file \
    binutils

RUN apt-get install -y --no-install-recommends python

COPY emscripten.sh /
RUN bash /emscripten.sh

COPY emscripten-entry.sh /
ENTRYPOINT ["/emscripten-entry.sh"]

ENV CARGO_TARGET_ASMJS_UNKNOWN_EMSCRIPTEN_RUNNER=node
