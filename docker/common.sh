#!/usr/bin/env bash

set -ex

apt-get update

apt-get install -y --no-install-recommends \
  autoconf \
  automake \
  binutils \
  ca-certificates \
  cmake \
  file \
  gcc \
  git \
  libc6-dev \
  libtool \
  m4 \
  make \
  pkg-config
