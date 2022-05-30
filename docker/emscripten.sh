#!/usr/bin/env bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    install_packages ca-certificates \
        curl \
        git \
        libxml2 \
        python

    cd /
    git clone https://github.com/emscripten-core/emsdk.git /emsdk-portable
    cd /emsdk-portable

    ./emsdk install 1.38.46-upstream
    ./emsdk activate 1.38.46-upstream

    # Compile and cache libc
    echo 'int main() {}' > a.c
    emcc a.c
    emcc -s BINARYEN=1 a.c
    echo -e '#include <iostream>\n void hello(){ std::cout << std::endl; }' > a.cpp
    emcc a.cpp
    emcc -s BINARYEN=1 a.cpp
    rm -f a.*

    # Make emsdk usable by any user
    chmod a+rwX -R "${EMSDK}"

    purge_packages

    rm "${0}"
}

main "${@}"
