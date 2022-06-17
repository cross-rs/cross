#!/usr/bin/env bash

ci_dir=$(dirname "${BASH_SOURCE[0]}")
ci_dir=$(realpath "${ci_dir}")
PROJECT_HOME=$(dirname "${ci_dir}")
export PROJECT_HOME
CARGO_TMP_DIR="${PROJECT_HOME}/target/tmp"
export CARGO_TMP_DIR

if [[ -n "${CROSS_CONTAINER_ENGINE}" ]]; then
  CROSS_ENGINE="${CROSS_CONTAINER_ENGINE}"
elif command -v docker >/dev/null 2>&1; then
  CROSS_ENGINE=docker
else
  CROSS_ENGINE=podman
fi
export CROSS_ENGINE

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

function mkcargotemp {
  local td=
  td="$CARGO_TMP_DIR"/$(mktemp -u "${@}" | xargs basename)
  mkdir -p "$td"
  echo '# Cargo.toml
  [workspace]
  members = ["'"$(basename "$td")"'"]
   ' > "$CARGO_TMP_DIR"/Cargo.toml
  echo "$td"
}
