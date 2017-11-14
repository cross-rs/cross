FROM ubuntu:14.04

COPY cross-apt-packages.sh /
RUN sh /cross-apt-packages.sh

COPY crosstool-ng.sh /
RUN sh /crosstool-ng.sh

COPY rustbuild.sh /
RUN sh /rustbuild.sh
USER rustbuild
WORKDIR /tmp


COPY arm-unknown-linux-gnueabihf/arm-linux-gnueabihf.config /tmp/
COPY arm-unknown-linux-gnueabihf/build-toolchains.sh /tmp/
RUN sh ./build-toolchains.sh

USER root

ENV PATH=$PATH:/x-tools/arm-unknown-linux-gnueabihf/bin
RUN ls /x-tools
RUN ls /x-tools/arm-unknown-linux-gnueabihf
RUN ls /x-tools/arm-unknown-linux-gnueabihf/bin

COPY xargo.sh /
RUN bash /xargo.sh

COPY openssl.sh qemu.sh /

RUN bash /openssl.sh linux-armv4 arm-unknown-linux-gnueabihf- && \
    bash /qemu.sh arm

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_RUNNER=qemu-arm \
    CC_arm_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    AR_arm_unknown_linux_gnueabihf=arm-unknown-linux-gnueabihf-ar \
    CXX_arm_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/arm-linux-gnueabihf \
    RUST_TEST_THREADS=1
