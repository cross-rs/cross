FROM ubuntu:16.04

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 mips64el

RUN apt-get install -y --no-install-recommends \
    gcc-mips64el-linux-gnuabi64 \
    libc6-dev-mips64el-cross

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux64-mips64 mips64el-linux-gnuabi64-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    g++-mips64el-linux-gnuabi64 \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_MIPS64EL_UNKNOWN_LINUX_GNUABI64_LINKER=mips64el-linux-gnuabi64-gcc \
    CC_mips64el_unknown_linux_gnuabi64=mips64el-linux-gnuabi64-gcc \
    CXX_mips64el_unknown_linux_gnuabi64=mips64el-linux-gnuabi64-g++ \
    OPENSSL_DIR=/openssl \
    QEMU_LD_PREFIX=/usr/mips64el-linux-gnuabi64 \
    RUST_TEST_THREADS=1
