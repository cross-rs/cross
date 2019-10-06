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
    git clone https://github.com/emscripten-core/emsdk.git /emsdk-portable

    export HOME=/emsdk-portable/

    ./emsdk install 1.38.46-upstream
    ./emsdk activate 1.38.46-upstream

    # Compile and cache libc
    source ./emsdk_env.sh
    echo "main(){}" > a.c
    emcc a.c
    echo -e "#include <iostream>\n void hello(){ std::cout << std::endl; }" > a.cpp
    emcc a.cpp
    rm -f a.*

    # Make emsdk usable by any user
    chmod a+rw -R /emsdk-portable/
    chmod a+x `find /emsdk-portable/ -executable -print` || true

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    rm $0
}

main "${@}"
