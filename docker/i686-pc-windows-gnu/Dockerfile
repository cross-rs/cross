# wine in ubuntu versions less than 17.04 hangs in the tests
FROM ubuntu:17.04

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

RUN dpkg --add-architecture i386 && apt-get update && \
    apt-get install -y --no-install-recommends \
        wine-stable \
        wine64 \
        wine32 \
        libz-mingw-w64-dev

# Build mingw tools using dwarf exceptions
COPY mingw.sh /
RUN bash mingw.sh

# run-detectors are responsible for calling the correct interpreter for exe
# files. For some reason it does not work inside a docker container (it works
# fine in the host). So we replace the usual paths of run-detectors to run wine
# directly. This only affects the guest, we are not messing up with the host.
#
# See /usr/share/doc/binfmt-support/detectors
RUN mkdir -p /usr/lib/binfmt-support/ && \
    rm -f /usr/lib/binfmt-support/run-detectors /usr/bin/run-detectors && \
    ln -s /usr/bin/wine /usr/lib/binfmt-support/run-detectors && \
    ln -s /usr/bin/wine /usr/bin/run-detectors

COPY windows-entry.sh /
ENTRYPOINT ["/windows-entry.sh"]

ENV CARGO_TARGET_I686_PC_WINDOWS_GNU_LINKER=i686-w64-mingw32-gcc \
    CARGO_TARGET_I686_PC_WINDOWS_GNU_RUNNER=wine \
    CC_i686_pc_windows_gnu=i686-w64-mingw32-gcc \
    CXX_i686_pc_windows_gnu=i686-w64-mingw32-g++
