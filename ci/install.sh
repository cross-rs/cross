set -ex

main() {
    curl https://sh.rustup.rs -sSf | \
        sh -s -- -y --default-toolchain $TRAVIS_RUST_VERSION

    if [ -f cache/$TARGET.tar ]; then
        docker load -i cache/$TARGET.tar
    fi
}

main
