FROM ubuntu:24.04 as cross-base
ENV DEBIAN_FRONTEND=noninteractive

COPY common.sh lib.sh /
RUN /common.sh

COPY cmake.sh /
RUN /cmake.sh

COPY xargo.sh /
RUN /xargo.sh

FROM cross-base as build

ARG TARGETPLATFORM
COPY zig.sh /
RUN /zig.sh $TARGETPLATFORM

# we don't export `BINDGEN_EXTRA_CLANG_ARGS`, `QEMU_LD_PREFIX`, or
# `PKG_CONFIG_PATH` since zig doesn't have a traditional sysroot structure,
# and we're not using standard, shared packages. none of the packages
# have runners either, since they do not ship with the required
# dynamic linker (`ld-linux-${arch}.so`).
ENV PATH=$PATH:/opt/zig
