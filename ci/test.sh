#!/usr/bin/env bash

set -x
set -euo pipefail

function retry {
  local tries="${TRIES-5}"
  local timeout="${TIMEOUT-1}"
  local try=0
  local exit_code=0

  while (( try < tries )); do
    if "${@}"; then
      return 0
    else
      exit_code=$?
    fi

    sleep "${timeout}"
    echo "Retrying ..." 1>&2
    try=$(( try + 1 ))
    timeout=$(( timeout * 2 ))
  done

  return ${exit_code}
}

main() {
    local td=

    retry cargo fetch
    cargo install --force --path .

    export QEMU_STRACE=1

    if (( ${STD:-0} )); then
        # test `cross check`
        td=$(mktemp -d)
        cargo init --lib --name foo "${td}"
        pushd "${td}"
        echo '#![no_std]' > src/lib.rs
        cross check --target "${TARGET}"
        popd
        rm -rf "${td}"
    else
        # `cross build` test for targets where `std` is not available
        td=$(mktemp -d)

        git clone \
            --depth 1 \
            --recursive \
            https://github.com/rust-lang-nursery/compiler-builtins "${td}"

        pushd "${td}"
        cat > Cross.toml <<EOF
[build]
xargo = true
EOF
        retry cargo fetch
        cross build --lib --target "${TARGET}"
        popd

        rm -rf "${td}"

        return
    fi

    # `cross build` test for the other targets
    if [[ "${TARGET}" == *-unknown-emscripten ]]; then
        td=$(mktemp -d)

        pushd "${td}"
        cargo init --lib --name foo .
        retry cargo fetch
        cross build --target "${TARGET}"
        popd

        rm -rf "${td}"
    elif [[ "${TARGET}" != thumb* ]]; then
        td=$(mktemp -d)

        pushd "${td}"
        # test that linking works
        cargo init --bin --name hello .
        retry cargo fetch
        cross build --target "${TARGET}"
        popd

        rm -rf "${td}"
    fi

    if (( ${RUN:-0} )); then
        # `cross test` test
        if (( ${DYLIB:-0} )); then
            td=$(mktemp -d)

            pushd "${td}"
            cargo init --lib --name foo .
            cross_test --target "${TARGET}"
            cross_bench --target "${TARGET}"
            popd

            rm -rf "${td}"
        fi

        # `cross run` test
        case "${TARGET}" in
            thumb*-none-eabi*)
                td=$(mktemp -d)

                git clone \
                    --depth 1 \
                    --recursive \
                    https://github.com/japaric/cortest "${td}"

                pushd "${td}"
                cross_run --target "${TARGET}" --example hello --release
                popd

                rm -rf "${td}"
            ;;
            *)
                td=$(mktemp -d)

                cargo init --bin --name hello "${td}"

                pushd "${td}"
                mkdir examples tests
                echo "fn main() { println!(\"Example!\"); }" > examples/e.rs
                echo "#[test] fn t() {}" > tests/t.rs
                cross_run --target "${TARGET}"
                cross_run --target "${TARGET}" --example e
                cross_test --target "${TARGET}"
                cross_bench --target "${TARGET}"
                popd

                rm -rf "${td}"
            ;;
        esac

    fi

    # Test C++ support
    if (( ${CPP:-0} )); then
        td="$(mktemp -d)"

        git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"

        pushd "${td}"
        retry cargo fetch
        if (( ${RUN:-0} )); then
            cross_run --target "${TARGET}"
        else
            cross build --target "${TARGET}"
        fi
        popd

        rm -rf "${td}"
    fi
}

cross_run() {
    if [[ -z "${RUNNERS:-}" ]]; then
        cross run "$@"
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > Cross.toml
            cross run "$@"
        done
    fi
}

cross_test() {
    if [[ -z "${RUNNERS:-}" ]]; then
        cross test "$@"
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > Cross.toml
            cross test "$@"
        done
    fi
}

cross_bench() {
    if [[ -z "${RUNNERS:-}" ]]; then
        cross bench "$@"
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > Cross.toml
            cross bench "$@"
        done
    fi
}

main
