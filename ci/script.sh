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
            cross build --manifest-path $td/Cargo.toml --target $TARGET

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

        cargo generate-lockfile \
                --manifest-path $td/Cargo.toml

        cross test \
                --manifest-path $td/Cargo.toml \
                --no-default-features \
                --target $TARGET

        rm -rf $td
    fi
}

main
