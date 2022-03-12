#!/usr/bin/env bash

set -x
set -euo pipefail

version="$(cargo metadata --format-version 1 --no-deps | jq --raw-output '.packages[0].version')"
images=()
push=false

for arg in "${@}"; do
  if [[ "${arg}" == --push ]]; then
    push=true
  else
    images+=("${arg}")
  fi
done

pushd docker

run() {
  local push="${1}"
  local build_args=()

  if "${push}"; then
    build_args+=(--push)
  fi

  local dockerfile="Dockerfile.${2}"
  local image_name="ghcr.io/cross-rs/${2}"

  local tags=()

  case "${GITHUB_REF_TYPE-}:${GITHUB_REF_NAME-}" in
    tag:v*)
      local tag_version="${GITHUB_REF_NAME##v}"

      if [[ "${tag_version}" == "${version}" ]]; then
        echo "Git tag does not match package version." >&2
        exit 1
      fi

      tags+=("${image_name}:${tag_version}")

      # Tag stable versions as latest.
      if ! [[ "${tag_version}" =~ alpha ]] && ! [[ "${tag_version}" =~ dev ]]; then
        tags+=("${image_name}:latest")
      fi
      ;;
    branch:*)
      tags+=("${image_name}:${GITHUB_REF_NAME}")
      ;;
    *)
      if "${push}"; then
        echo "Refusing to push without tag or branch." >&2
        exit 1
      fi

      # Local development.
      tags+=("${image_name}:local")
      build_args+=(--load)
      ;;
  esac

  build_args+=(--pull --cache-from 'type=gha' --cache-to 'type=gha,mode=max')

  for tag in "${tags[@]}"; do
    build_args+=(--tag "${tag}")
  done

  docker buildx build "${build_args[@]}" -f "${dockerfile}" --progress plain .
}

if [[ "${#images[@]}" -eq 0 ]]; then
  for t in Dockerfile.*; do
    run "${push}" "${t##Dockerfile.}"
  done
else
  for image in "${images[@]}"; do
    run "${push}" "${image}"
  done
fi
