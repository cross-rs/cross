#!/usr/bin/env bash
# shellcheck disable=SC2086,SC1091,SC1090

set -x
set -eo pipefail

# NOTE: "${@}" is an unbound variable for bash 3.2, which is the
# installed version on macOS. likewise, "${var[@]}" is an unbound
# error if var is an empty array.

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
. "${ci_dir}"/shared.sh

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
    CROSS=$(binary_path cross "${PROJECT_HOME}" debug)
    export CROSS=("${CROSS}")
    export CROSS_FLAGS="-v"
    if (( ${BUILD_STD:-0} )); then
        # use build-std instead of xargo, due to xargo being
        # maintenance-only. build-std requires a nightly compiler
        rustup toolchain add nightly
        CROSS_FLAGS="${CROSS_FLAGS} -Zbuild-std"
        CROSS+=("+nightly")
        if [[ "${TARGET}" == *"mips"* ]]; then
            # workaround for https://github.com/cross-rs/cross/issues/1322 & https://github.com/rust-lang/rust/issues/108835
            [[ ! "$RUSTFLAGS" =~ opt-level ]] && export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C opt-level=1"
        fi
    elif ! (( ${STD:-0} )); then
        # don't use xargo: should have native support just from rustc
        rustup toolchain add nightly
        CROSS+=("+nightly")
    fi

    if (( ${STD:-0} )); then
        # test `cross check`
        td=$(mkcargotemp -d)
        cargo init --lib --name foo "${td}"
        pushd "${td}"
        echo '#![no_std]' > src/lib.rs
        "${CROSS[@]}" check --target "${TARGET}" ${CROSS_FLAGS}
        popd
        rm -rf "${td}"
    else
        # `cross build` test for targets where `std` is not available
        td=$(mkcargotemp -d)

        git clone \
            --depth 1 \
            --recursive \
            https://github.com/rust-lang-nursery/compiler-builtins "${td}"

        pushd "${td}"
        retry cargo fetch
        # don't use xargo: should have native support just from rustc
        rustup toolchain add nightly
        cross_build --lib --target "${TARGET}"
        popd

        rm -rf "${td}"
    fi

    # `cross build` test for the other targets
    if [[ "${TARGET}" == *-unknown-emscripten ]]; then
        td=$(mkcargotemp -d)

        pushd "${td}"
        cargo init --lib --name foo .
        retry cargo fetch
        cross_build --target "${TARGET}"
        popd

        rm -rf "${td}"
    # thumb targets are tested in later steps
    elif [[ "${TARGET}" != thumb* ]]; then
        td=$(mkcargotemp -d)

        pushd "${td}"
        # test that linking works
        cargo init --bin --name hello .
        retry cargo fetch
        cross_build --target "${TARGET}"
        popd

        rm -rf "${td}"
    fi

    if (( ${RUN:-0} )); then
        # `cross test` test
        if (( ${DYLIB:-0} )); then
            td=$(mkcargotemp -d)

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
                td=$(mkcargotemp -d)

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
                td=$(mkcargotemp -d)

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
                td=$(mkcargotemp -d)
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

    # Test C++ support in a no_std context
    if (( ${CPP:-0} )); then
        td="$(mkcargotemp -d)"

        git clone --depth 1 https://github.com/cross-rs/rust-cpp-accumulate "${td}"

        pushd "${td}"
        retry cargo fetch
        cross_build --target "${TARGET}"
        popd

        rm -rf "${td}"
    fi

    # Test C++ support
    if (( ${STD:-0} )) && (( ${CPP:-0} )); then
        td="$(mkcargotemp -d)"

        git clone --depth 1 https://github.com/cross-rs/rust-cpp-hello-word "${td}"

        pushd "${td}"
        retry cargo fetch
        if (( ${RUN:-0} )); then
            cross_run --target "${TARGET}"
        else
            cross_build --target "${TARGET}"
        fi
        popd

        rm -rf "${td}"
    fi

    # special tests for a shared C runtime, since we disable the shared c++ runtime
    # https://github.com/cross-rs/cross/issues/902
    if [[ "${TARGET}" == *-linux-musl* ]]; then
        td=$(mkcargotemp -d)

        pushd "${td}"
        cargo init --bin --name hello .
        retry cargo fetch
        RUSTFLAGS="$RUSTFLAGS -C target-feature=-crt-static" \
            cross_build --target "${TARGET}"
        popd

        rm -rf "${td}"
    fi

    # test cmake support
    td="$(mkcargotemp -d)"

    git clone \
        --recursive \
        --depth 1 \
        https://github.com/cross-rs/rust-cmake-hello-world "${td}"

    pushd "${td}"
    retry cargo fetch
    if [[ "${TARGET}" == "arm-linux-androideabi" ]]; then
        # ARMv5te isn't supported anymore by Android, which produces missing
        # symbol errors with re2 like `__libcpp_signed_lock_free`.
        cross_run --target "${TARGET}" --features=tryrun
    elif (( ${STD:-0} )) && (( ${RUN:-0} )) && (( ${CPP:-0} )); then
        cross_run --target "${TARGET}" --features=re2,tryrun
    elif (( ${STD:-0} )) && (( ${CPP:-0} )); then
        cross_build --target "${TARGET}" --features=re2
    elif (( ${STD:-0} )) && (( ${RUN:-0} )); then
        cross_run --target "${TARGET}" --features=tryrun
    elif (( ${STD:-0} )); then
        cross_build --target "${TARGET}" --features=tryrun
    else
        cross_build --lib --target "${TARGET}"
    fi
    popd

    rm -rf "${td}"

    # test running binaries with cleared environment
    # Command is not implemented for wasm32-unknown-emscripten
    if (( ${RUN:-0} )) && [[ "${TARGET}" != "wasm32-unknown-emscripten" ]]; then
        td="$(mkcargotemp -d)"
        pushd "${td}"
        cargo init --bin --name foo .
        mkdir src/bin
        upper_target=$(echo "${TARGET}" | tr '[:lower:]' '[:upper:]' | tr '-' '_')
        cat <<EOF > src/bin/launch.rs
fn main() {
    let runner = std::env::var("CARGO_TARGET_${upper_target}_RUNNER");
    let mut command = if let Ok(runner) = runner {
        runner.split(' ').map(str::to_string).collect()
    } else {
        vec![]
    };
    let executable = format!("/target/${TARGET}/debug/foo{}", std::env::consts::EXE_SUFFIX);
    command.push(executable.to_string());
    let status = dbg!(std::process::Command::new(&command[0])
        .args(&command[1..])
        .env_clear()) // drop all environment variables
    .status()
    .unwrap();
    std::process::exit(status.code().unwrap());
}
EOF
        cross_build --target "${TARGET}"
        cross_run --target "${TARGET}" --bin launch
        popd
        rm -rf "${td}"
    fi
}

cross_build() {
    "${CROSS[@]}" build "$@" ${CROSS_FLAGS}
}

cross_run() {
    if [[ -z "${RUNNERS:-}" ]]; then
        "${CROSS[@]}" run "$@" ${CROSS_FLAGS}
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > "${CARGO_TMP_DIR}"/Cross.toml
            "${CROSS[@]}" run "$@" ${CROSS_FLAGS}
        done
    fi
}

cross_test() {
    if [[ -z "${RUNNERS:-}" ]]; then
        "${CROSS[@]}" test "$@" ${CROSS_FLAGS}
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > "${CARGO_TMP_DIR}"/Cross.toml
            "${CROSS[@]}" test "$@" ${CROSS_FLAGS}
        done
    fi
}

cross_bench() {
    if [[ -z "${RUNNERS:-}" ]]; then
        "${CROSS[@]}" bench "$@" ${CROSS_FLAGS}
    else
        for runner in ${RUNNERS}; do
            echo -e "[target.${TARGET}]\nrunner = \"${runner}\"" > "${CARGO_TMP_DIR}"/Cross.toml
            "${CROSS[@]}" bench "$@" ${CROSS_FLAGS}
        done
    fi
}

main
