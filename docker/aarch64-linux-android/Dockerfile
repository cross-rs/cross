FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

# api 21 the minimum for aarch64
RUN apt-get install -y --no-install-recommends \
    curl \
    unzip \
    python && \
    curl -O https://dl.google.com/android/repository/android-ndk-r13b-linux-x86_64.zip && \
    unzip -q android-ndk-r13b-linux-x86_64.zip && \
    ./android-ndk-r13b/build/tools/make_standalone_toolchain.py \
        --install-dir /android-ndk \
        --arch arm64 \
        --api 21 && \
    rm -rf ./android-ndk-r13b-linux-x86_64.zip ./android-ndk-r13b && \
    apt-get purge --auto-remove -y curl unzip python

ENV PATH=$PATH:/android-ndk/bin

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j linux-generic64 aarch64-linux-android- -mandroid -fomit-frame-pointer

RUN apt-get install -y --no-install-recommends \
    curl && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

ENV CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android-gcc \
    CC_AARCH64_linux_android=aarch64-linux-android-gcc \
    CXX_AARCH64_linux_android=aarch64-linux-android-g++ \
    DEP_Z_ROOT=/android-ndk/sysroot/usr/ \
    OPENSSL_STATIC=1 \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1 \
    HOME=/tmp/
