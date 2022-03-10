#!/usr/bin/env bash

set -e

# shellcheck source=/dev/null
source "${HOME}/esp-rust.sh"

exec "$@"
