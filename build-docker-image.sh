set -ex

run() {
    cd docker
    docker build -t japaric/$1 -f ${1}/Dockerfile .
    cd ..
}

if [ -z $1 ]; then
    for t in `ls docker/`; do
        if [ -d docker/$t ]; then
            run $t
        fi
    done
else
    run $1
fi
