set -ex

main() {
    local dependencies=(
        ca-certificates
        curl
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
    curl -L https://nodejs.org/dist/v8.0.0/node-v8.0.0-linux-x64.tar.xz | \
        tar -xJ

    # Clean up
    apt-get purge --auto-remove -y ${purge_list[@]}

    rm $0
}

main "${@}"
