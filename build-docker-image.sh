#!/bin/bash

set -ex

cd docker

run() {
    docker build \
           -t "rustembedded/cross:${1}" \
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
