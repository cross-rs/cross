set -ex

main() {
    curl https://sh.rustup.rs -sSf | \
        sh -s -- -y --default-toolchain $TRAVIS_RUST_VERSION

    if [ $TRAVIS_BRANCH != master ]; then
        docker run \
               --privileged \
               --rm \
               -it \
               ubuntu:16.04 \
               sh -c "apt-get update && apt-get install --no-install-recommends -y binfmt-support qemu-user-static"
    fi

    if [ -f cache/$TARGET.tar ]; then
        docker load -i cache/$TARGET.tar
    fi
}

main
