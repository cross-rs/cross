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
RUN apt-get install -y --no-install-recommends \
    gcc-arm-none-eabi \
    libnewlib-arm-none-eabi && \
    bash /qemu.sh arm

ENV QEMU_CPU=cortex-m4 \
    CARGO_TARGET_THUMBV7EM_NONE_EABIHF_RUNNER=qemu-arm
