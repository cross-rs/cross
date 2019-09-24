#!/usr/bin/env bash

set -eux

cargo install --path . --force
cross build --target "${TARGET}" --release

rm -rf "${BUILD_BINARIESDIRECTORY}"
mkdir "${BUILD_BINARIESDIRECTORY}"

if [[ -f "target/${TARGET}/release/cross.exe" ]]; then
  mv "target/${TARGET}/release/cross.exe" "${BUILD_BINARIESDIRECTORY}/"
else
  mv "target/${TARGET}/release/cross" "${BUILD_BINARIESDIRECTORY}/"
fi
