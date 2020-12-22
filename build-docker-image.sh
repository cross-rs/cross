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

  if [[ "${PULL:-1}" = "1" ]]; then
    docker build ${cache_from_args[@]+"${cache_from_args[@]}"} --pull -t "${image_name}" -f "${dockerfile}" .
  else
    docker build ${cache_from_args[@]+"${cache_from_args[@]}"} -t "${image_name}" -f "${dockerfile}" .
  fi

  if ! [[ "${version}" =~ alpha ]] && ! [[ "${version}" =~ dev ]]; then
    local versioned_image_name="${image_name}-${version}"
    docker tag "${image_name}" "${versioned_image_name}"
  fi

  if [[ "${1}" = "context" ]]; then
    # complex pipelines read better top to bottom
    # shellcheck disable=SC2002
    cat Dockerfile.context \
      | sed -nE "s/^## ?//p" \
      | sed "s@rustembedded/cross:context@${versioned_image_name}@" \
      > Dockerfile.context.doctest
    PULL=0 run context.doctest
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
