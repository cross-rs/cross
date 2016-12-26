set -ex

main() {
    if [ $TRAVIS_BRANCH != master ] && [ -z $TRAVIS_TAG ]; then
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
        docker push japaric/$TARGET:$tag
    fi
}

main
