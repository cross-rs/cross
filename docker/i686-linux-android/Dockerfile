FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

COPY android-ndk.sh /
RUN bash /android-ndk.sh x86 21
ENV PATH=$PATH:/android-ndk/bin

COPY openssl.sh /
RUN bash /openssl.sh android-x86 i686-linux-android-

RUN apt-get install -y --no-install-recommends \
    curl && \
    curl -LSfs http://japaric.github.io/trust/install.sh | \
    sh -s -- --git japaric/xargo --tag v0.3.5 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

# Libz is distributed in the android ndk, but for some unknown reason it is not
# found in the build process of some crates, so we explicit set the DEP_Z_ROOT
ENV CARGO_TARGET_I686_LINUX_ANDROID_LINKER=i686-linux-android-gcc \
    CC_i686_linux_android=i686-linux-android-gcc \
    CXX_i686_linux_android=i686-linux-android-g++ \
    DEP_Z_ROOT=/android-ndk/sysroot/usr/ \
    OPENSSL_STATIC=1 \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    RUST_TEST_THREADS=1 \
    HOME=/tmp/
