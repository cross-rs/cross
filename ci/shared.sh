#!/usr/bin/env bash

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
