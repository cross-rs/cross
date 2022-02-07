#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
  local version=1.14
  local arch=amd64
  gpg --batch --keyserver hkps://keys.openpgp.org --recv-keys B42F6819007F00F88E364FD4036A9C25BF357DD4
  curl -o /usr/local/bin/gosu -SL "https://github.com/tianon/gosu/releases/download/${version}/gosu-${arch}"
  curl -o /usr/local/bin/gosu.asc -SL "https://github.com/tianon/gosu/releases/download/${version}/gosu-${arch}.asc"
  gpg --batch --verify /usr/local/bin/gosu.asc /usr/local/bin/gosu
  rm /usr/local/bin/gosu.asc
  rm -r /root/.gnupg/
  chmod +x /usr/local/bin/gosu
  # Verify that the binary works
  gosu nobody true
}

main "${@}"
