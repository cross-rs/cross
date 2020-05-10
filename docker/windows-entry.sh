#!/usr/bin/env bash

set -e

export HOME=/tmp/home
mkdir -p "${HOME}"

# Initialize the wine prefix (virtual windows installation)
export WINEPREFIX=/tmp/wine
mkdir -p "${WINEPREFIX}"
# FIXME: Make the wine prefix initialization faster
wineboot &> /dev/null

# Put libstdc++ and some other mingw dlls in WINEPATH
# This must work for x86_64 and i686
P1="$(dirname "$(find /usr -name libwinpthread-1.dll)")"

WINEPATH="$(ls -d /usr/lib/gcc/*-w64-mingw32/*posix);${P1}"
export WINEPATH

exec "$@"
