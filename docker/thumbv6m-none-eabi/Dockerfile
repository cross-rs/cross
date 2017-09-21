FROM ubuntu:16.04

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
RUN apt-get install -y --no-install-recommends \
    gcc-arm-none-eabi \
    libnewlib-arm-none-eabi && \
    bash /qemu.sh arm

ENV QEMU_CPU=cortex-m3 \
    CARGO_TARGET_THUMBV6M_NONE_EABI_RUNNER=qemu-arm
