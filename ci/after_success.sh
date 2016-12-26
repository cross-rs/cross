set -ex

main() {
    if [ $TRAVIS_BRANCH != master ]; then
        return
    fi

    set +x
    docker login \
           -p $DOCKER_PASS \
           -u $DOCKER_USER
    set -x

    docker push japaric/$TARGET
}

main
