#!/usr/bin/env bash

set -e

echo "$DOCKER_PASSWORD" | docker login -u "$DOCKER_USERNAME" --password-stdin

image_name="rustembedded/cross:$TARGET"

if [ ! -z "$TRAVIS_TAG" ]; then
  versioned_image_name="$image_name-${TRAVIS_TAG##v}"
  docker tag "$image_name" "$versioned_image_name"
  docker push "$versioned_image_name"
fi

docker push "$image_name"
