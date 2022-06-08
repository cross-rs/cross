#!/usr/bin/env bash

set -x
set -euo pipefail

version="$(cargo metadata --format-version 1 --no-deps | jq --raw-output '.packages[0].version')"
targets=()
push=false

for arg in "${@}"; do
  if [[ "${arg}" == --push ]]; then
    push=true
  else
    targets+=("${arg}")
  fi
done

pushd docker

run() {
  local push="${1}"
  local build_args=()

  if "${push}"; then
    build_args+=(--push)
  else
    build_args+=(--load)
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
      if ! [[ "${tag_version}" =~ -.* ]]; then
        tags+=("${image_name}:latest")
      fi
      ;;
    branch:*)
      # Tag active branch as edge.
      tags+=("${image_name}:${GITHUB_REF_NAME}")
      if ! [[ "${GITHUB_REF_NAME-}" =~ staging ]] && ! [[ "${GITHUB_REF_NAME-}" =~ trying ]]; then
        tags+=("${image_name}:edge")
      fi
      ;;
    *)
      if "${push}"; then
        echo "Refusing to push without tag or branch." >&2
        exit 1
      fi

      # Local development.
      tags+=("${image_name}:local")
      ;;
  esac

  build_args+=(
    --pull
    --cache-from "type=registry,ref=${image_name}:main"
  )

  if "${push}"; then
    build_args+=(--cache-to 'type=inline')
  fi

  for tag in "${tags[@]}"; do
    build_args+=(--tag "${tag}")
  done

  if [ -n "${LABELS:-}" ]; then
    local labels
    mapfile -t labels -d '' <<< "${LABELS}"
    for label in "${labels[@]}"; do
      build_args+=(--label "${label}")
    done
  fi

  docker buildx build "${build_args[@]}" -f "${dockerfile}" --progress plain .
  docker inspect "${tags[0]}" | jq -C .[0].Config.Labels
  if [[ -n "${GITHUB_ACTIONS-}" ]]; then
    echo "::set-output name=image::${tags[0]}"
  fi
}

if [[ "${#targets[@]}" -eq 0 ]]; then
  for dockerfile in Dockerfile.*; do
    target="${dockerfile##Dockerfile.}"
    run "${push}" "${target}"
  done
else
  for target in "${targets[@]}"; do
    run "${push}" "${target}"
  done
fi
