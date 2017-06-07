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

    ./emsdk update
    ./emsdk install latest
    ./emsdk activate latest

    # Make emsdk usable by any user
    cp /root/.emscripten /emsdk-portable
    chmod a+r -R /emsdk-portable/
    chmod a+x /emsdk-portable/emsdk
    chmod a+xw /emsdk-portable/

    # Compile and cache libc
    source ./emsdk_env.sh
    echo "main(){}" > a.c
    emcc a.c
    emcc -s BINARYEN=1 a.c
    echo -e "#include <iostream>\n void hello(){ std::cout << std::endl; }" > a.cpp
    emcc a.cpp
    emcc -s BINARYEN=1 a.cpp
    rm -f a.*
    chmod a+rw -R /emsdk-portable/.emscripten_cache/
    rm /emsdk-portable/.emscripten_cache.lock

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    rm $0
}

main "${@}"
