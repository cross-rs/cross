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
    sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

COPY qemu.sh /
RUN apt-get install -y --no-install-recommends \
    gcc-arm-none-eabi \
    libnewlib-arm-none-eabi && \
    bash /qemu.sh 2.8.0 arm

ENV QEMU_CPU=cortex-m3
