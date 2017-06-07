#!/bin/bash

set -e

export HOME=/tmp/

cp /emsdk-portable/.emscripten $HOME/

source /emsdk-portable/emsdk_env.sh &> /dev/null

exec "$@"
