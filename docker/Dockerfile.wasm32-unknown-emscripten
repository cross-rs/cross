FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config \
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

COPY node.sh /
RUN bash /node.sh

COPY emscripten-entry.sh /
ENTRYPOINT ["/emscripten-entry.sh"]

COPY node-wasm /usr/local/bin/
ENV CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_RUNNER=node-wasm
