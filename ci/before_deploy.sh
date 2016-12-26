set -ex

main() {
    local src=$(pwd) \
          target=x86_64-unknown-linux-musl \
          td=$(mktemp -d)

    rustup target add $target

    cargo rustc --target $target --release -- -C lto

    cp target/$target/release/cross $td/

    cd $td
    tar czf $src/cross-$TRAVIS_TAG-$target.tar.gz *
    cd $src

    rm -rf $td
}

main
