#!/bin/bash

set -e

export HOME=/emsdk-portable/

source /emsdk-portable/emsdk_env.sh &> /dev/null

exec "$@"
