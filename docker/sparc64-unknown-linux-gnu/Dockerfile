FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

COPY openssl.sh qemu.sh /
RUN apt-get install -y --no-install-recommends \
    g++-sparc64-linux-gnu \
    libc6-dev-sparc64-cross && \
    bash /openssl.sh 1.0.2j linux64-sparcv9 sparc64-linux-gnu- && \
    bash /qemu.sh 2.8.0 sparc64

RUN apt-get install -y --no-install-recommends \
    curl && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

ENV CARGO_TARGET_SPARC64_UNKNOWN_LINUX_GNU_LINKER=sparc64-linux-gnu-gcc \
    CC_sparc64_unknown_linux_gnu=sparc64-linux-gnu-gcc \
    CXX_sparc64_unknown_linux_gnu=sparc64-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/sparc64-linux-gnu \
    RUST_TEST_THREADS=1
