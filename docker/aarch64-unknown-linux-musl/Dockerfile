FROM ubuntu:18.04

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
    binutils \
    openssl \
    libssl-dev \
    llvm-dev \
    libclang-dev \
    clang

COPY xargo.sh /
RUN bash /xargo.sh

COPY qemu.sh /
RUN bash /qemu.sh aarch64

COPY musl.sh /
RUN bash /musl.sh TARGET=aarch64-linux-musl

COPY openssl.sh /
RUN bash /openssl.sh linux-aarch64 aarch64-linux-musl-

# Allows qemu run dynamic linked binaries
RUN ln -sf \
    /usr/local/aarch64-linux-musl/lib/libc.so \
    /usr/local/aarch64-linux-musl/lib/ld-musl-aarch64.so.1
ENV QEMU_LD_PREFIX=/usr/local/aarch64-linux-musl

# Workaround for https://github.com/rust-lang/rust/issues/46651
ENV RUSTFLAGS="-C link-arg=-lgcc"

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUNNER=qemu-aarch64 \
    CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc \
    CXX_aarch64_unknown_linux_musl=aarch64-linux-musl-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1
