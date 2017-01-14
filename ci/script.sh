set -ex

main() {
    local td=

    if [ $TRAVIS_OS_NAME = linux ]; then
        ./build-docker-image.sh $TARGET
    fi

    if [ $TRAVIS_BRANCH = master ] || [ ! -z $TRAVIS_TAG ]; then
        return
    fi

    cargo install --path .

    # `cross build` test for targets where `std` is not available
    case $TARGET in
        thumbv*-none-eabi*)
            td=$(mktemp -d)

            git clone \
                --depth 1 \
                --recursive \
                https://github.com/japaric/cortest $td

            pushd $td
            cross run --target $TARGET --example hello
            popd

            rm -rf $td

            return
        ;;
    esac

    # `cross build` test for targets where `std` is not available
    case $TARGET in
        sparc64-* | \
            thumbv*-none-eabi* | \
            x86_64-unknown-dragonfly)
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
            cross build --features c --lib --target $TARGET
            popd

            rm -rf $td

            return
            ;;
    esac

    # `cross build` test for the other targets
    if [ $TARGET = i686-apple-darwin ] || [ $TARGET = i686-unknown-linux-musl ]; then
        td=$(mktemp -d)

        git clone --depth 1 https://github.com/japaric/xargo $td

        pushd $td
        cross build --target $TARGET
        popd

        rm -rf $td
    else
        td=$(mktemp -d)

        git clone --depth 1 https://github.com/rust-lang/cargo $td

        pushd $td
        cross build --target $TARGET
        popd

        rm -rf $td
    fi

    # `cross test` / `cross run` test for the other targets
    # NOTE(s390x) japaric/cross#3
    # NOTE(*-musl) can't test compiler-builtins because that crate needs
    # cdylibs and musl targets don't support cdylibs
    # NOTE(*-*bsd) no `cross test` support for BSD targets
    # NOTE(sparc64-*) no `std` available
    case $TARGET in
        i686-unknown-freebsd | \
            i686-unknown-linux-musl | \
            s390x-unknown-linux-gnu | \
            sparc64-unknown-linux-gnu | \
            x86_64-unknown-freebsd | \
            x86_64-unknown-linux-musl | \
            x86_64-unknown-netbsd)
        ;;
        *)
            td=$(mktemp -d)

            git clone \
                --depth 1 \
                --recursive \
                https://github.com/rust-lang-nursery/compiler-builtins \
                $td

            pushd $td
            cross test \
                  --no-default-features \
                  --target $TARGET
            popd

            rm -rf $td

            td=$(mktemp -d)

            cargo init --bin --name hello $td

            pushd $td
            cross run --target $TARGET
            popd

            rm -rf $td
        ;;
    esac

    # Test C++ support
    case $TARGET in
        *-unknown-*bsd | \
            *-unknown-linux-musl)
            ;;
        *)
            td=$(mktemp -d)

            git clone --depth 1 https://github.com/japaric/hellopp $td

            pushd $td
            if [ $TARGET = s390x-unknown-linux-gnu ]; then
                cross build --target $TARGET
            else
                cross run --target $TARGET
            fi
            popd

            rm -rf $td
            ;;
    esac

    # Test openssl compatibility
    if [ $TRAVIS_OS_NAME = linux ] && [ ! -z "$OPENSSL_INCLUDE_PATH"] && [ ! -z "$OPENSSL_LIB_PATH" ]; then
        td=$(mktemp -d)

        pushd $td
        cargo clone openssl-sys --vers 0.5.5
        cd openssl-sys
        cross build --target $TARGET
        popd

        rm -rf $td
    fi
}

main
