set -ex

main() {
    local src=$(pwd) \
          td=$(mktemp -d)

    cargo install --path . -f

    cross rustc --target $TARGET --release -- -C lto

    cp target/$TARGET/release/cross $td/

    cd $td
    tar czf $src/cross-$TRAVIS_TAG-$TARGET.tar.gz *
    cd $src

    rm -rf $td
}

main
