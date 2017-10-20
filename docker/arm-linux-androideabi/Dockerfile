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
RUN bash /qemu.sh arm android

COPY android-ndk.sh /
RUN bash /android-ndk.sh arm 21
ENV PATH=$PATH:/android-ndk/bin

COPY openssl.sh /
RUN bash /openssl.sh android arm-linux-androideabi-

COPY android-system.sh /
RUN bash /android-system.sh arm

# Libz is distributed in the android ndk, but for some unknown reason it is not
# found in the build process of some crates, so we explicit set the DEP_Z_ROOT
ENV CARGO_TARGET_ARM_LINUX_ANDROIDEABI_LINKER=arm-linux-androideabi-gcc \
    CARGO_TARGET_ARM_LINUX_ANDROIDEABI_RUNNER=qemu-arm \
    CC_arm_linux_androideabi=arm-linux-androideabi-gcc \
    CXX_arm_linux_androideabi=arm-linux-androideabi-g++ \
    DEP_Z_ROOT=/android-ndk/sysroot/usr/ \
    OPENSSL_STATIC=1 \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1 \
    HOME=/tmp/ \
    TMPDIR=/tmp/ \
    ANDROID_ROOT=/system \
    ANDROID_DATA=/
