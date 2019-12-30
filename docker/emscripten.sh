#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    local dependencies=(
        ca-certificates
        curl
        git
        libxml2
        python
    )

    apt-get update
    local purge_list=()
    for dep in ${dependencies[@]}; do
        if ! dpkg -L $dep; then
            apt-get install --no-install-recommends -y $dep
            purge_list+=( $dep )
        fi
    done

    cd /
    git clone https://github.com/emscripten-core/emsdk.git /emsdk-portable
    cd /emsdk-portable

    ./emsdk install 1.38.46-upstream
    ./emsdk activate 1.38.46-upstream

    # Compile and cache libc
    echo 'int main() {}' > a.c
    emcc a.c
    emcc -s BINARYEN=1 a.c
    echo -e "#include <iostream>\n void hello(){ std::cout << std::endl; }" > a.cpp
    emcc a.cpp
    emcc -s BINARYEN=1 a.cpp
    rm -f a.*

    # Make emsdk usable by any user
    chmod a+rwX -R "${EMSDK}"

    if (( ${#purge_list[@]} )); then
      apt-get purge --auto-remove -y ${purge_list[@]}
    fi

    rm $0
}

main "${@}"
