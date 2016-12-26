set -ex

main() {
    mkdir -p cache

    docker save \
           japaric/$TARGET:latest \
           $(docker history -q japaric/$TARGET:latest | grep -v \<missing\>) \
           > cache/$TARGET.tar
}

main
