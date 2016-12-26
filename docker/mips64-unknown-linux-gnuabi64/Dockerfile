FROM ubuntu:16.04

COPY override.sh /
RUN bash /override.sh curl-sys 0.3.6

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 mips64

RUN apt-get install -y --no-install-recommends \
    gcc-mips64-linux-gnuabi64 \
    libc6-dev-mips64-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux64-mips64 mips64-linux-gnuabi64-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_MIPS64_UNKNOWN_LINUX_GNUABI64_LINKER=mips64-linux-gnuabi64-gcc \
    CC_mips64_unknown_linux_gnuabi64=mips64-linux-gnuabi64-gcc \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/mips64-linux-gnuabi64 \
    RUST_TEST_THREADS=1
