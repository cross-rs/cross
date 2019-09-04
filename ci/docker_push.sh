#!/usr/bin/env bash

set -eux

image_name="rustembedded/cross:${TARGET}"

if [[ "${TAG-}" =~ ^v.* ]] && ! [[ "${TAG}" =~ alpha ]] && ! [[ "${TAG}" =~ dev ]]; then
  version="${TAG##v}"
  versioned_image_name="${image_name}-${version}"
  docker tag "${image_name}" "${versioned_image_name}"
  docker push "${versioned_image_name}"
fi

docker push "${image_name}"
