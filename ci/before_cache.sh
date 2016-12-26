set -ex

main() {
    mkdir -p cache

    docker save \
           japaric/$TARGET \
           $(docker history -q japaric/$TARGET | grep -v \<missing\>) \
           > cache/$TARGET.tar
}

main
