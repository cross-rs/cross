#!/usr/bin/env bash

set -x
set -euo pipefail

version="$(cargo metadata --format-version 1 | jq --raw-output '.packages[] | select(.name == "cross") | .version')"
aarch64_unsupported = (
  aarch64-linux-android
  arm-linux-androideabi
  armv7-linux-androideabi
  asmjs-unknown-emscripten # todo
  i686-linux-android
  i686-pc-windows-gnu
  powerpc-unknown-linux-gnu
  powerpc64-unknown-linux-gnu
  powerpc64le-unknown-linux-gnu
  sparc64-unknown-linux-gnu
  sparcv9-sun-solaris # todo
  wasm32-unknown-emscripten # todo
  x86_64-linux-android
  x86_64-pc-windows-gnu
  x86_64-sun-solaris
)

cd docker

run() {
  local dockerfile="Dockerfile.${1}"
  local image_name="rustembedded/cross:${1}"
  local cache_from_args=()

  docker build ${cache_from_args[@]+"${cache_from_args[@]}"} -t "${image_name}" -f "${dockerfile}" .

  if ! [[ "${version}" =~ alpha ]] && ! [[ "${version}" =~ dev ]]; then
    local versioned_image_name="${image_name}-${version}"
    docker tag "${image_name}" "${versioned_image_name}"
  fi
}

if [[ -z "${*}" ]]; then
  for t in Dockerfile.*; do
    target = "${t##Dockerfile.}"
    run "${target}"
  done
else
  for image in "${@}"; do
    run "${image}"
  done
fi
