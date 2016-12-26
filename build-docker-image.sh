set -ex

run() {
    local tag=
    if [ -z $TRAVIS_TAG ]; then
        tag=latest
    else
        tag=${TRAVIS_TAG#v}
    fi

    docker build -t japaric/$1:$tag -f docker/${1}/Dockerfile docker
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
