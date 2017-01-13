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
    sh -s -- --git japaric/xargo --tag v0.3.3 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

COPY override.sh /
RUN bash /override.sh curl-sys 0.3.6

COPY openssl.sh qemu.sh /
RUN apt-get install -y --no-install-recommends \
    g++-powerpc64-linux-gnu \
    libc6-dev-ppc64-cross && \
    bash /openssl.sh 1.0.2j linux-ppc64 powerpc64-linux-gnu- && \
    bash /qemu.sh 2.7.1 ppc64

ENV CARGO_TARGET_POWERPC64_UNKNOWN_LINUX_GNU_LINKER=powerpc64-linux-gnu-gcc \
    CC_powerpc64_unknown_linux_gnu=powerpc64-linux-gnu-gcc \
    CXX_powerpc64_unknown_linux_gnu=powerpc64-linux-gnu-g++ \
    OPENSSL_INCLUDE_DIR=/openssl/include OPENSSL_LIB_DIR=/openssl/lib \
    QEMU_LD_PREFIX=/usr/powerpc64-linux-gnu \
    RUST_TEST_THREADS=1
