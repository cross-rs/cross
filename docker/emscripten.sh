#!/usr/bin/env bash

set -x
set -euo pipefail

main() {
    apt-get install --no-install-recommends --assume-yes \
        python3 \
        python
        git \
        llvm \
        clang \
        lld \
        nodejs \
        npm \
        binaryen

    git clone https://github.com/emscripten-core/emsdk.git /emsdk-portable
    cd /emsdk-portable

    ./emsdk install emscripten-tag-1.38.31-64bit
    ./emsdk activate emscripten-tag-1.38.31-64bit

    echo "LLVM_ROOT = '/usr/bin'" >> .emscripten
    echo "BINARYEN_ROOT = '/usr/bin'" >> .emscripten

cat <<EOF > /entrypoint
#!/bin/bash

source /emsdk-portable/emsdk_env.sh >/dev/null
/bin/bash
EOF
    chmod +x /entrypoint

    # # Compile and cache libc
    # echo 'int main() {}' > a.c
    # emcc a.c
    # emcc -s BINARYEN=1 a.c
    # echo -e '#include <iostream>\n void hello(){ std::cout << std::endl; }' > a.cpp
    # emcc a.cpp
    # emcc -s BINARYEN=1 a.cpp
    # rm -f a.*

    # # Make emsdk usable by any user
    # chmod a+rwX -R "${EMSDK}"
}

main "${@}"
