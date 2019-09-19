#!/usr/bin/env bash

set -ex

main() {
    local dependencies=(
        ca-certificates
        cmake
        curl
        git
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
    curl -L https://s3.amazonaws.com/mozilla-games/emscripten/releases/emsdk-portable.tar.gz | \
        tar -xz
    cd /emsdk-portable

    export HOME=/emsdk-portable/

    ./emsdk update
    ./emsdk install sdk-1.38.15-64bit
    ./emsdk activate sdk-1.38.15-64bit

    # Compile and cache libc
    source ./emsdk_env.sh
    echo "main(){}" > a.c
    emcc a.c
    emcc -s BINARYEN=1 a.c
    echo -e "#include <iostream>\n void hello(){ std::cout << std::endl; }" > a.cpp
    emcc a.cpp
    emcc -s BINARYEN=1 a.cpp
    rm -f a.*

    # Make emsdk usable by any user
    chmod a+rw -R /emsdk-portable/
    find /emsdk-portable/ -executable -print0 | xargs -0 chmod a+x

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    rm $0
}

main "${@}"
