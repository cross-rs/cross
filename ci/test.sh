#!/usr/bin/env bash

set -x
set -euo pipefail

function retry {
  local tries=${TRIES-5}
  local timeout=${TIMEOUT-1}
  local try=0
  local exit_code=0

  while (( ${try} < ${tries} )); do
    if "${@}"; then
      return 0
    else
      exit_code=$?
    fi

    sleep ${timeout}
    echo "Retrying ..." 1>&2
    try=$(( try + 1 ))
    timeout=$(( timeout * 2 ))
  done

  return ${exit_code}
}

main() {
    local td=

    if [ "${OS}" = linux ]; then
        ./build-docker-image.sh $TARGET
    fi

    if [ "${BRANCH-}" = master ] || [[ "${TAG-}" =~ ^v.* ]]; then
        return
    fi

    retry cargo fetch
    cargo install --force --path .

    export QEMU_STRACE=1

    # test `cross check`
    if [ ! -z $STD ]; then
        td=$(mktemp -d)
        cargo init --lib --name foo $td
        pushd $td
        echo '#![no_std]' > src/lib.rs
        cross check --target $TARGET
        popd
        rm -rf $td
    fi

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
        retry cargo fetch
        cross build --lib --target $TARGET
        popd

        rm -rf $td

        return
    fi

    # `cross build` test for the other targets
    if [[ "$TARGET" == *-unknown-emscripten ]]; then
        td=$(mktemp -d)

        pushd $td
        cargo init --lib --name foo .
        retry cargo fetch
        cross build --target $TARGET
        popd

        rm -rf $td
    elif [[ "$TARGET" != thumb* ]]; then
        td=$(mktemp -d)

        pushd $td
        # test that linking works
        cargo init --bin --name hello .
        retry cargo fetch
        cross build --target $TARGET
        popd

        rm -rf $td
    fi

    if [ $RUN ]; then
        # `cross test` test
        if [ $DYLIB ]; then
            td=$(mktemp -d)

            pushd $td
            cargo init --lib --name foo .
            cross_test --target $TARGET
            cross_bench --target $TARGET
            popd

            rm -rf $td
        fi

        # `cross run` test
        case $TARGET in
            thumb*-none-eabi*)
                td=$(mktemp -d)

                git clone \
                    --depth 1 \
                    --recursive \
                    https://github.com/japaric/cortest $td

                pushd $td
                cross_run --target $TARGET --example hello --release
                popd

                rm -rf $td
            ;;
            *)
                td=$(mktemp -d)

                cargo init --bin --name hello $td

                pushd $td
                mkdir examples tests
                echo "fn main() { println!(\"Example!\"); }" > examples/e.rs
                echo "#[test] fn t() {}" > tests/t.rs
                cross_run --target $TARGET
                cross_run --target $TARGET --example e
                cross_test --target $TARGET
                cross_bench --target $TARGET
                popd

                rm -rf $td
            ;;
        esac

    fi

    # Test C++ support
    if [ $CPP ]; then
        td=$(mktemp -d)

        git clone --depth 1 https://github.com/japaric/hellopp $td

        pushd $td
        cargo update -p gcc
        retry cargo fetch
        if [ $RUN ]; then
            cross_run --target $TARGET
        else
            cross build --target $TARGET
        fi
        popd

        rm -rf $td
    fi
}

cross_run() {
    if [ -z "$RUNNERS" ]; then
        cross run "$@"
    else
        for runner in $RUNNERS; do
            echo -e "[target.$TARGET]\nrunner = \"$runner\"" > Cross.toml
            cross run "$@"
        done
    fi
}

cross_test() {
    if [ -z "$RUNNERS" ]; then
        cross test "$@"
    else
        for runner in $RUNNERS; do
            echo -e "[target.$TARGET]\nrunner = \"$runner\"" > Cross.toml
            cross test "$@"
        done
    fi
}

cross_bench() {
    if [ -z "$RUNNERS" ]; then
        cross bench "$@"
    else
        for runner in $RUNNERS; do
            echo -e "[target.$TARGET]\nrunner = \"$runner\"" > Cross.toml
            cross bench "$@"
        done
    fi
}

main
