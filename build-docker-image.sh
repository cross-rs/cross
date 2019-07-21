#!/bin/bash

set -ex

cd docker

run() {
    tag="${TRAVIS_TAG##v}"
    docker build \
           -t "rustembedded/cross:${1}${tag:+-$tag}" \
           -f "Dockerfile.${1}" \
           .
}

if [ -z $1 ]; then
    for t in Dockerfile.*; do
        run "${t##Dockerfile.}"
    done
else
    run $1
fi
