set -ex

main() {
    ./build-docker-image.sh $TARGET

    cargo install --path .

    if [ $TRAVIS_BRANCH = master ]; then
        return
    fi

    # NOTE(case) japaric/cross#4
    case $TARGET in
        mips-unknown-linux-gnu | \
            mips64-unknown-linux-gnuabi64 | \
            powerpc64-unknown-linux-gnu)
        ;;
        *)
            local td=$(mktemp -d)

            git clone --depth 1 https://github.com/rust-lang/cargo $td

            pushd $td
            cross build --target $TARGET
            popd

            rm -rf $td
            ;;
    esac

    # NOTE(if) japaric/cross#3
    if [ $TARGET != s390x-unknown-linux-gnu ]; then
        local td=$(mktemp -d)

        git clone \
            --depth 1 \
            --recursive \
            https://github.com/rust-lang-nursery/compiler-builtins \
            $td

        pushd $td
        cargo generate-lockfile
        cross test \
              --no-default-features \
              --target $TARGET
        popd

        rm -rf $td
    fi
}

main
