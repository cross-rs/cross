FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

COPY override.sh /
RUN bash /override.sh curl-sys 0.3.8

COPY openssl.sh qemu.sh /
RUN apt-get install -y --no-install-recommends \
    g++-mips-linux-gnu \
    libc6-dev-mips-cross && \
    bash /openssl.sh 1.0.2j linux-mips32 mips-linux-gnu- && \
    bash /qemu.sh 2.8.0 mips

RUN apt-get install -y --no-install-recommends \
    curl && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

ENV CARGO_TARGET_MIPS_UNKNOWN_LINUX_GNU_LINKER=mips-linux-gnu-gcc \
    CC_mips_unknown_linux_gnu=mips-linux-gnu-gcc \
    CXX_mips_unknown_linux_gnu=mips-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/mips-linux-gnu \
    RUST_TEST_THREADS=1
