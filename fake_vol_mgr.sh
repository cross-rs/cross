#!/bin/sh

# TODO: move this logic into Cross, not a shell script
# Run with something like `TOOLCHAIN=nightly-2017-07-07 ./fake_vol_mgr.sh`
#
# Ideally this would be run before executing any cross command to populate
# the Rust environment (if necessary), and $XARGO_DIR, $CARGO_DIR, and $RUST_DIR
# would be mounted to /xargo, /cargo, and /rust in the compilation environment

set -eux

export DEST_DIR=/tmp/solocross
export TOOLCHAIN_DIR="$DEST_DIR/$TOOLCHAIN"

export XARGO_DIR="$DEST_DIR"/xargo
export CARGO_DIR="$TOOLCHAIN_DIR"/cargo
export RUST_DIR="$TOOLCHAIN_DIR"/rust

# TODO: if TOOLCHAIN_DIR and XARGO_DIR exists, bail happy,
# maybe a bit better checking that the environments aren't broken

mkdir -p "$DEST_DIR"
mkdir -p "$XARGO_DIR"
mkdir -p "$CARGO_DIR"
mkdir -p "$RUST_DIR"

docker build \
    -t volmgr \
    -f ./volume_manager/Dockerfile \
    ./volume_manager

docker run \
    -e RUST_TOOLCHAIN="$TOOLCHAIN" \
    -v "$XARGO_DIR":/xargo \
    -v "$CARGO_DIR":/cargo \
    -v "$RUST_DIR":/rust \
    -t volmgr
