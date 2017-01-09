FROM ubuntu:16.04

COPY qemu.sh /
RUN bash /qemu.sh 2.8.0 arm

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    gcc \
    gcc-arm-none-eabi \
    libc6-dev \
    libnewlib-dev \
    libnewlib-arm-none-eabi && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.1 --target x86_64-unknown-linux-gnu --to /usr/bin

ENV QEMU_CPU=cortex-m3
