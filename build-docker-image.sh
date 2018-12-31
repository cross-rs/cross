#!/bin/sh

set -ex

run() {
    docker build \
           -t japaric/$1:${TRAVIS_TAG:-latest} \
           -f docker/${1}/Dockerfile \
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
