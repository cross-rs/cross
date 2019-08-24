set -ex

cargo install --path . --force
cross rustc --target $TARGET --release -- -C lto

tar czf "cross-$TRAVIS_TAG-$TARGET.tar.gz" -C "target/$TARGET/release" cross
