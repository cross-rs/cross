FROM ubuntu:16.04

COPY netbsd.sh /
RUN bash /netbsd.sh

COPY openssl.sh /
RUN bash /openssl.sh 1.0.2j BSD-x86_64 x86_64-unknown-netbsd-

RUN apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    gcc \
    libc6-dev \
    make \
    pkg-config

ENV CARGO_TARGET_X86_64_UNKNOWN_NETBSD_LINKER=x86_64-unknown-netbsd-gcc \
    CC_x86_64_unknown_netbsd=x86_64-unknown-netbsd-gcc \
    CXX_x86_64_unknown_netbsd=x86_64-unknown-netbsd-g++ \
    OPENSSL_DIR=/openssl
