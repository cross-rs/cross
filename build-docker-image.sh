#!/bin/bash

set -ex

run() {
    tag="${TRAVIS_TAG##v}"
    docker build \
           -t "rustembedded/cross:${1}${tag:+-$tag}" \
           -f "docker/${1}/Dockerfile" \
           docker
}

if [ -z $1 ]; then
    for t in `ls docker/`; do
        if [ -d docker/$t ]; then
            run $t
        fi
    done
else
    run $1
fi
