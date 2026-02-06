#!/usr/bin/env bash
set -ex

tf=$(mktemp -t stash.XXX.$$)

cleanup() {
    git apply --allow-empty --whitespace=nowarn < "$tf" && git stash drop -q
    rm "$tf"
}

git diff --full-index --binary > "$tf"
git stash -q --keep-index

trap cleanup EXIT

cargo xtask test --verbose
