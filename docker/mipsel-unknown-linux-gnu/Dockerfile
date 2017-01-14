FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    curl \
    gcc \
    libc6-dev \
    make \
    pkg-config && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.3 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

COPY openssl.sh qemu.sh /
RUN apt-get install -y --no-install-recommends \
    g++-mipsel-linux-gnu \
    libc6-dev-mipsel-cross && \
    bash /openssl.sh 1.0.2j linux-mips32 mipsel-linux-gnu- && \
    bash /qemu.sh 2.8.0 mipsel

ENV CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_GNU_LINKER=mipsel-linux-gnu-gcc \
    CC_mipsel_unknown_linux_gnu=mipsel-linux-gnu-gcc \
    CXX_mipsel_unknown_linux_gnu=mipsel-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/mipsel-linux-gnu \
    RUST_TEST_THREADS=1
