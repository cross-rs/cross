#!/usr/bin/env bash

set -euo pipefail

export HOME=/emsdk-portable/

source /emsdk-portable/emsdk_env.sh &> /dev/null

exec "$@"
