set -ex

main() {
    local td=

    if [ "$TRAVIS_OS_NAME" = linux ]; then
        ./build-docker-image.sh $TARGET
    fi

    if [ "$TRAVIS_BRANCH" = master ] || [ ! -z "$TRAVIS_TAG" ]; then
        return
    fi

    cargo install --force --path .

    export QEMU_STRACE=1

    # test `cross check`
    if [ ! -z $STD ]; then
        td=$(mktemp -d)
        cargo init --lib --name foo $td
        pushd $td
        echo '#![no_std]' > src/lib.rs
        cross +nightly check --target $TARGET
        popd
        rm -rf $td
    fi

    # `cross run` test for thumb targets
    case $TARGET in
        thumb*-none-eabi*)
            td=$(mktemp -d)

            git clone \
                --depth 1 \
                --recursive \
                https://github.com/japaric/cortest $td

            pushd $td
            cross +nightly run --target $TARGET --example hello --release
            popd

            rm -rf $td
        ;;
    esac

    # `cross build` test for targets where `std` is not available
    if [ -z "$STD" ]; then
        td=$(mktemp -d)

        git clone \
            --depth 1 \
            --recursive \
            https://github.com/rust-lang-nursery/compiler-builtins $td

        pushd $td
        cat > Cross.toml <<EOF
[build]
xargo = true
EOF
        cross +nightly build --features c --lib --target $TARGET
        popd

        rm -rf $td

        return
    fi

    # `cross build` test for the other targets
    if [ $OPENSSL ]; then
        td=$(mktemp -d)

        git clone --depth 1 https://github.com/rust-lang/cargo $td

        pushd $td
        cross +nightly build --target $TARGET
        popd

        rm -rf $td
    elif [ "$TARGET" = "asmjs-unknown-emscripten" -o \
           "$TARGET" = "wasm32-unknown-emscripten" ]; then
        td=$(mktemp -d)

        git clone --depth 1 https://github.com/bluss/rust-itertools $td

        pushd $td
        cross +nightly build --target $TARGET
        popd

        rm -rf $td
    else
        td=$(mktemp -d)

        git clone --depth 1 https://github.com/japaric/xargo $td

        pushd $td
        cross +nightly build --target $TARGET
        popd

        rm -rf $td
    fi

    if [ $RUN ]; then
        # `cross test` test
        if [ $DYLIB ]; then
            td=$(mktemp -d)

            git clone \
                --depth 1 \
                --recursive \
                https://github.com/rust-lang-nursery/compiler-builtins \
                $td

            pushd $td
            cross +nightly test \
                  --no-default-features \
                  --features "gen-tests mangled-names" \
                  --target $TARGET
            popd

            rm -rf $td
        fi

        # `cross run` test
        td=$(mktemp -d)

        cargo init --bin --name hello $td

        pushd $td
        mkdir examples tests
        echo "fn main() { println!(\"Example!\"); }" > examples/e.rs
        echo "#[test] fn t() {}" > tests/t.rs
        cross +nightly run --target $TARGET
        cross +nightly run --target $TARGET --example e
        cross +nightly test --target $TARGET
        popd

        rm -rf $td
    fi

    # Test C++ support
    if [ $CPP ]; then
        td=$(mktemp -d)

        git clone --depth 1 https://github.com/japaric/hellopp $td

        pushd $td
        cargo update -p gcc
        if [ $RUN ]; then
            cross +nightly run --target $TARGET
        else
            cross +nightly build --target $TARGET
        fi
        popd

        rm -rf $td
    fi

    # Test openssl compatibility
    if [ $OPENSSL ]; then
        td=$(mktemp -d)

        # If tag name v$OPENSSL fails we try openssl-sys-v$OPENSSL
        git clone \
            --depth 1 \
            --branch v$OPENSSL \
            https://github.com/sfackler/rust-openssl $td || \
        git clone \
            --depth 1 \
            --branch openssl-sys-v$OPENSSL \
            https://github.com/sfackler/rust-openssl $td

        pushd $td
        # avoid problems building openssl-sys in a virtual workspace
        rm -f Cargo.toml
        cd openssl-sys && cross +nightly build --target $TARGET
        popd

        rm -rf $td
    fi
}

main
