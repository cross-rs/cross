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
    sh -s -- --git japaric/xargo --tag v0.3.4 --target x86_64-unknown-linux-gnu --to /usr/bin && \
    apt-get purge --auto-remove -y curl

COPY netbsd.sh openssl.sh /
RUN bash /netbsd.sh && \
    bash /openssl.sh 1.0.2j BSD-x86_64 x86_64-unknown-netbsd-

ENV CARGO_TARGET_X86_64_UNKNOWN_NETBSD_LINKER=x86_64-unknown-netbsd-gcc \
    CC_x86_64_unknown_netbsd=x86_64-unknown-netbsd-gcc \
    OPENSSL_DIR=/openssl \
    OPENSSL_INCLUDE_DIR=/openssl/include \
    OPENSSL_LIB_DIR=/openssl/lib
