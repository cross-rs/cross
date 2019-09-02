#!/usr/bin/env bash

set -eux

cargo install --path . --force
cross rustc --target "${TARGET}" --release -- -C lto

tar czf "${BUILD_ARTIFACTSTAGINGDIRECTORY}/cross-${TAG}-${TARGET}.tar.gz" -C "target/${TARGET}/release" cross
