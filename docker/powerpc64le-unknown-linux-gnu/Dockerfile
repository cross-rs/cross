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

COPY openssl.sh qemu.sh /
RUN apt-get install -y --no-install-recommends \
    g++-powerpc64le-linux-gnu \
    libc6-dev-ppc64el-cross && \
    bash /openssl.sh 1.0.2j linux-ppc64le powerpc64le-linux-gnu- && \
    bash /qemu.sh 2.7.1 ppc64le

ENV CARGO_TARGET_POWERPC64LE_UNKNOWN_LINUX_GNU_LINKER=powerpc64le-linux-gnu-gcc \
    CC_powerpc64le_unknown_linux_gnu=powerpc64le-linux-gnu-gcc \
    CXX_powerpc64le_unknown_linux_gnu=powerpc64le-linux-gnu-g++ \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/powerpc64le-linux-gnu \
    RUST_TEST_THREADS=1
