#!/usr/bin/env bash

set -x
set -euo pipefail

version="$(cargo metadata --format-version 1 | jq --raw-output '.packages[] | select(.name == "cross") | .version')"

cd docker

run() {
  local dockerfile="Dockerfile.${1}"
  local image_name="rustembedded/cross:${1}"
  local cache_from_args=()

  if ! docker image inspect "${image_name}" &>/dev/null; then
    if docker pull "${image_name}"; then
      cache_from_args=(--cache-from "${image_name}")
    fi
  fi

  if grep -i centos "${dockerfile}" >/dev/null 2>/dev/null; then
      # build debian sysroot in a separate ubuntu container
      # (only done for x86-linux-gnu ATM)
      rm -rf qemu
      mkdir qemu
      cp linux-image.sh qemu/
      docker run \
        --rm \
        -v "$(pwd)/qemu:/qemu:z" \
        -w /qemu \
        -i \
        -t "ubuntu:16.04" \
        sh -c "./linux-image.sh x86_64; chown -R $(id -u):$(id -g) /qemu"
  fi

  docker build ${cache_from_args[@]+"${cache_from_args[@]}"} --pull -t "${image_name}" -f "${dockerfile}" .

  if ! [[ "${version}" =~ alpha ]] && ! [[ "${version}" =~ dev ]]; then
    local versioned_image_name="${image_name}-${version}"
    docker tag "${image_name}" "${versioned_image_name}"
  fi
}

if [[ -z "${*}" ]]; then
  for t in Dockerfile.*; do
    run "${t##Dockerfile.}"
  done
else
  for image in "${@}"; do
    run "${image}"
  done
fi
