#!/usr/bin/env bash

set -eux

image_name="rustembedded/cross:${TARGET}"

if [[ "${TAG-}" =~ ^v.* ]] && ! [[ "${TAG}" =~ alpha ]] && ! [[ "${TAG}" =~ dev ]]; then
  docker push "${versioned_image_name}"
fi

docker push "${image_name}"
