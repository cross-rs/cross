set -ex

main() {
    if [ $TRAVIS_BRANCH != master ] && [ -z $TRAVIS_TAG ]; then
        return
    fi

    if [ $TRAVIS_OS_NAME = linux ]; then
        set +x
        docker login \
               -p "$DOCKER_PASS" \
               -u "$DOCKER_USER"
        set -x

        docker push japaric/$TARGET:${TRAVIS_TAG:-latest}
    fi
}

main
