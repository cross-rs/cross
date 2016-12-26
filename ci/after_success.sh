set -ex

main() {
    if [ $TRAVIS_BRANCH != master ]; then
        return
    fi

    set +x
    docker login \
           -p "$DOCKER_PASS" \
           -u "$DOCKER_USER"
    set -x

    if [ -z $TRAVIS_TAG ]; then
        docker push japaric/$TARGET
    else
        local tag=${TRAVIS_TAG#v}
        docker tag japaric/$TARGET japaric/$TARGET:$tag
        docker push japaric/$TARGET:$tag
    fi
}

main
