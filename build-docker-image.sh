#!/usr/bin/env bash

set -ex

cd docker

run() {
  local dockerfile="Dockerfile.${1}"
  local image="rustembedded/cross:${1}"

  docker pull "${image}" || true
  docker build --pull --cache-from "${image}" -t "${image}" -f "${dockerfile}" .
}

if [ -z "${1}" ]; then
  for t in Dockerfile.*; do
    run "${t##Dockerfile.}"
  done
else
  run "${1}"
fi
