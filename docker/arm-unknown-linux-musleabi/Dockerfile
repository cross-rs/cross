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
    gcc-arm-linux-gnueabi

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABI_LINKER=arm-linux-gnueabi-gcc \
    CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABI_RUNNER=qemu-arm \
    CC_arm_unknown_linux_musleabi=arm-linux-gnueabi-gcc \
    RUST_TEST_THREADS=1
