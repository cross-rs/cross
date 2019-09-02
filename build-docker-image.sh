#!/usr/bin/env bash

set -ex

cd docker

run() {
  local dockerfile="Dockerfile.${1}"
  local image="rustembedded/cross:${1}"

  if docker pull "${image}"; then
    cache_from_args=(--cache-from "${image}")
  fi

  docker build ${cache_from_args[@]} --pull -t "${image}" -f "${dockerfile}" .
}

if [ -z "${1}" ]; then
  for t in Dockerfile.*; do
    run "${t##Dockerfile.}"
  done
else
  run "${1}"
fi
