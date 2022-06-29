#!/usr/bin/env bash
# shellcheck disable=SC2086

set -x
set -euo pipefail

# NOTE: "${@}" is an unbound variable for bash 3.2, which is the
# installed version on macOS. likewise, "${var[@]}" is an unbound
# error if var is an empty array.

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
project_home=$(dirname "${ci_dir}")

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

workspace_test() {
  "${CROSS[@]}" build --target "${TARGET}" --workspace "$@" ${CROSS_FLAGS}
  "${CROSS[@]}" run --target "${TARGET}" -p binary "$@" ${CROSS_FLAGS}
  "${CROSS[@]}" run --target "${TARGET}" --bin dependencies \
    --features=dependencies "$@" ${CROSS_FLAGS}
}

main() {
    local td=

    retry cargo fetch
    cargo build

    # Unset RUSTFLAGS
    export RUSTFLAGS=""

    export QEMU_STRACE=1

    # ensure we have the proper toolchain and optional rust flags
    export CROSS=("${project_home}/target/debug/cross")
    export CROSS_FLAGS=""
    if (( ${BUILD_STD:-0} )); then
        # use build-std instead of xargo, due to xargo being
        # maintenance-only. build-std requires a nightly compiler
        rustup toolchain add nightly
        CROSS_FLAGS="${CROSS_FLAGS} -Zbuild-std"
        CROSS+=("+nightly")
    elif ! (( ${STD:-0} )); then
        # don't use xargo: should have native support just from rustc
        rustup toolchain add nightly
        CROSS+=("+nightly")
    fi

    if (( ${STD:-0} )); then
        # test `cross check`
        td=$(mktemp -d)
        cargo init --lib --name foo "${td}"
        pushd "${td}"
        echo '#![no_std]' > src/lib.rs
        "${CROSS[@]}" check --target "${TARGET}" ${CROSS_FLAGS}
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
        retry cargo fetch
        # don't use xargo: should have native support just from rustc
        rustup toolchain add nightly
        "${CROSS[@]}" build --lib --target "${TARGET}" ${CROSS_FLAGS}
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
        "${CROSS[@]}" build --target "${TARGET}" ${CROSS_FLAGS}
        popd

        rm -rf "${td}"
    elif [[ "${TARGET}" != thumb* ]]; then
        td=$(mktemp -d)

        pushd "${td}"
        # test that linking works
        cargo init --bin --name hello .
        retry cargo fetch
        "${CROSS[@]}" build --target "${TARGET}" ${CROSS_FLAGS}
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
                td=$(mktemp -d)
                git clone \
                    --depth 1 \
                    --recursive \
                    https://github.com/cross-rs/test-workspace "${td}"

                pushd "${td}"
                TARGET="${TARGET}" workspace_test --manifest-path="./workspace/Cargo.toml"
                pushd "workspace"
                TARGET="${TARGET}" workspace_test
                pushd "binary"
                "${CROSS[@]}" run --target "${TARGET}" ${CROSS_FLAGS}
                popd
                popd
                popd
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
            "${CROSS[@]}" build --target "${TARGET}" ${CROSS_FLAGS}
        fi
        popd

        rm -rf "${td}"
    fi
}

cross_run() {
    if [[ -z "${RUNNERS:-}" ]]; then
        "${CROSS[@]}" run "$@" ${CROSS_FLAGS}
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > Cross.toml
            "${CROSS[@]}" run "$@" ${CROSS_FLAGS}
        done
    fi
}

cross_test() {
    if [[ -z "${RUNNERS:-}" ]]; then
        "${CROSS[@]}" test "$@" ${CROSS_FLAGS}
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > Cross.toml
            "${CROSS[@]}" test "$@" ${CROSS_FLAGS}
        done
    fi
}

cross_bench() {
    if [[ -z "${RUNNERS:-}" ]]; then
        "${CROSS[@]}" bench "$@" ${CROSS_FLAGS}
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > Cross.toml
            "${CROSS[@]}" bench "$@" ${CROSS_FLAGS}
        done
    fi
}

main
