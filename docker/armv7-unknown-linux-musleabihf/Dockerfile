FROM ubuntu:18.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

COPY xargo.sh /
RUN bash /xargo.sh

COPY qemu.sh /
RUN bash /qemu.sh arm

RUN apt-get install -y --no-install-recommends \
    gcc-arm-linux-gnueabihf

ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_RUNNER=qemu-arm \
    CC_armv7_unknown_linux_musleabihf=arm-linux-gnueabihf-gcc \
    RUST_TEST_THREADS=1
