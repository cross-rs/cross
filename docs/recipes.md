<!--toc:start-->
- [OpenSSL](#openssl)
  - [Vendored](#vendored)
  - [Pre-build](#pre-build)
  - [Custom dockerfile](#custom-dockerfile)
- [sccache](#sccache)
- [Redoxer](#redoxer)
- [vcpkg, Meson, and Conan](#vcpkg-meson-and-conan)
- [Using Clang and Software Collections on CentOS7](#using-clang-and-software-collections-on-centos7)
<!--toc:end-->

This contains recipes for common logic use cases.


# OpenSSL

You can either use the vendored or system packages for the
[openssl](https://crates.io/crates/openssl) crate. See
[openssl-certs](https://github.com/cross-rs/wiki_assets/tree/main/Recipes/openssl-certs)
for a working project.

## Vendored

Use the vendored feature of the openssl crate by adding the following to your
dependencies in `Cargo.toml`:

```toml,cargo
openssl = { version = "0.10", features = ["vendored"] }
```

## Pre-build

To install OpenSSL in an image with `apt-get` available add the following to
your [Cross
configuration](./config_file.md):

```toml
[target.x86_64-unknown-linux-gnu]
pre-build = [
    "dpkg --add-architecture $CROSS_DEB_ARCH",
    "apt-get update && apt-get install --assume-yes libssl-dev:$CROSS_DEB_ARCH"
]
```

## Custom dockerfile

A sample Dockerfile for `aarch64` with OpenSSL support is:

```Dockerfile
FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:edge
RUN dpkg --add-architecture arm64
RUN apt-get update && apt-get install --assume-yes libssl-dev:arm64
```

Build this image and use it, as is described extensively in [Custom
Images](./custom_images.md).


# sccache

sccache support can be done either by `sccache` from source or using a pre-built binary. See [sccache](https://github.com/cross-rs/wiki_assets/tree/main/Recipes/sccache) for a working project using pre-build hooks.

1. Create a script to [install](#sccache-install-script) sccache in the image, either from a [pre-built binary](#sccache-prebuilt-binary) or [from source](#sccache-from-source).
2. Extend a [Dockerfile](#sccache-dockerfile) to install sccache in the image.
3. Passthrough the appropriate environment variables in [Cross.toml](#sccache-cross-toml) when using sccache.

<h3 id="sccache-install-script">Install Script</h3>

First, we need a script to copy into our image as `sccache.sh` (make sure the script is executable).

<h4 id="sccache-prebuilt-binary">Pre-Built Binary</h4>

```bash
#!/bin/bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local triple
    local tag
    local td
    local url="https://github.com/mozilla/sccache"
    triple="${1}"

    install_packages unzip tar

    # Download our package, then install our binary.
    td="$(mktemp -d)"
    pushd "${td}"
    tag=$(git ls-remote --tags --refs --exit-code \
        "${url}" \
        | cut -d/ -f3 \
        | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' \
        | sort --version-sort \
        | tail -n1)
    curl -LSfs "${url}/releases/download/${tag}/sccache-${tag}-${triple}.tar.gz" \
        -o sccache.tar.gz
    tar -xvf sccache.tar.gz
    rm sccache.tar.gz
    cp "sccache-${tag}-${triple}/sccache" "/usr/bin/sccache"
    chmod +x "/usr/bin/sccache"

    # clean up our install
    purge_packages
    popd
    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
```

<h4 id="sccache-from-source">From Source</h4>

When installing from source, we can toggle various features, however it is highly recommended to use the vendored OpenSSL.

```bash
#!/bin/bash

set -x
set -euo pipefail

# shellcheck disable=SC1091
. lib.sh

main() {
    local triple
    local tag
    local td
    local url="https://github.com/mozilla/sccache"
    triple="${1}"

    install_packages ca-certificates curl unzip

    # install rust and cargo to build sccache
    export RUSTUP_HOME=/tmp/rustup
    export CARGO_HOME=/tmp/cargo
    curl --retry 3 -sSfL https://sh.rustup.rs -o rustup-init.sh
    sh rustup-init.sh -y --no-modify-path
    rm rustup-init.sh
    export PATH="${CARGO_HOME}/bin:${PATH}"
    rustup target add "${triple}"

    # download the source code from the latest sccache release
    td="$(mktemp -d)"
    pushd "${td}"
    tag=$(git ls-remote --tags --refs --exit-code \
        "${url}" \
        | cut -d/ -f3 \
        | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' \
        | sort --version-sort \
        | tail -n1)
    curl -LSfs "${url}/archive/refs/tags/${tag}.zip" \
        -o sccache.zip
    unzip sccache.zip
    mv "sccache-${tag//v/}" sccache
    rm sccache.zip

    # build from source for the desired architecture
    # you can also use additional features here
    cd sccache
    cargo build --release --target "${triple}" \
        --features=all,"openssl/vendored"
    cp "target/${triple}/release/sccache" "/usr/bin/sccache"

    # clean up our install
    rm -r "${RUSTUP_HOME}" "${CARGO_HOME}"
    purge_packages
    popd
    rm -rf "${td}"
    rm "${0}"
}

main "${@}"
```

<h3 id="sccache-dockerfile">Dockerfile</h3>

Next, extend our Dockerfile and build our image, saved as `Dockerfile.${target}`, where `${target}` is replaced by our desired target (such as `x86_64-unknown-linux-musl`).

```Dockerfile
FROM ghcr.io/cross-rs/${target}:main
ARG DEBIAN_FRONTEND=noninteractive

COPY sccache.sh /
RUN /sccache.sh x86_64-unknown-linux-musl

ENV RUSTC_WRAPPER="/usr/bin/sccache"
```

Build our Docker image with:

```bash
docker build --tag ${target}:sccache \
    --file Dockerfile.${target} .
```

<h3 id="sccache-cross-toml">Cross.toml</h3>

Now, we need to passthrough our environment variables and ensure they're exported when running cross. In `Cross.toml`, define:

```toml
[target.${target}]
image = "${target}:sccache"

[build.env]
passthrough = [
    "SCCACHE_ERROR_LOG",
    "SCCACHE_LOG",
    "SCCACHE_AZURE_CONNECTION_STRING",
    "SCCACHE_AZURE_BLOB_CONTAINER",
    "SCCACHE_DIR",
]
```

<h3 >Building with sccache</h3>

Finally, we can run cross with our `sccache` environment variables defined using `cross`:

```bash
SCCACHE_LOG=trace SCCACHE_DIR=/path/to/sccache/cache \
    cross build --target "${target}" --verbose
```

# Redoxer

Redoxer support can be done by installing the necessary dependencies, Redoxer, and the Redoxer toolchain in a custom image. See [redoxer](https://github.com/cross-rs/wiki_assets/tree/main/Recipes/redoxer) for a working project using a custom Dockerfile.

Please note that this requires a base Ubuntu version of 20.04, and therefore needs you to build the images with [newer Linux versions](https://github.com/cross-rs/cross/wiki/FAQ#newer-linux-versions).

# vcpkg, Meson, and Conan

Often C++ projects have complex build systems, due to a myriad of dependencies, competing build systems, and the lack of a built-in package manager. Some of the most popular build systems include GNU Make, CMake, and [Meson](https://mesonbuild.com/), and the two most popular package managers are [vcpkg](https://vcpkg.io/en/index.html) and [Conan](https://conan.io/). We have an entire [project](https://github.com/cross-rs/wiki_assets/tree/main/Recipes/vcpkg) with builds using CMake + Conan, Meson + Conan, and CMake + vcpkg.

An example of building a project with an external `zlib` dependency using Meson and Conan is as follows. First, we create our Conan dependency file:

**conanfile.py**

```python
from conans import ConanFile, Meson

class ZlibExec(ConanFile):
    name = "zlibexec"
    version = "0.1"
    settings = "os", "compiler", "build_type", "arch"
    generators = "cmake", "pkg_config"
    requires = "zlib/1.2.11"

    def build(self):
        meson = Meson(self)
        meson.configure(build_folder="build")
        meson.build()
```

Next, we need our Meson build file:

**meson.build**

```meson
project('zlibexec', 'cpp')
executable('zlibexec', 'zlib.cc', dependencies: dependency('zlib'))
```

Now, we need to build our project:

```bash
mkdir build && cd build
conan install .. --build
meson ..
conan build ..
```

To make this magic happen, the project contains [Dockerfiles](https://github.com/cross-rs/wiki_assets/blob/main/Recipes/vcpkg/aarch64.Dockerfile) with Meson, Conan, and vcpkg installed where the CMake toolchains and Meson configurations automatically cross-compile for the desire architecture. These images are [automatically](https://github.com/cross-rs/wiki_assets/blob/main/Recipes/vcpkg/Cross.toml) built when running `cross` via [pre-build hooks](https://github.com/cross-rs/cross/wiki/Configuration#custom-images). In order to integrate these builds with Rust, rather than invoking the `meson` or `cmake` commands directly, you should use [meson-rs](https://docs.rs/meson/1.0.0/meson/) and [cmake-rs](https://docs.rs/cmake/latest/cmake/) to configure and build the projects.

# Using Clang and Software Collections on CentOS7

In order to use Clang on CentOS 7, you must both install the SCL repository, the LLVM toolset, and set the necessary paths to clang and LLVM. A sample Dockerfile is as follows:

```Dockerfile
FROM ghcr.io/cross-rs/x86_64-unknown-linux-gnu:main-centos

RUN yum update -y && \
    yum install centos-release-scl -y && \
    yum install llvm-toolset-7 -y

ENV LIBCLANG_PATH=/opt/rh/llvm-toolset-7/root/usr/lib64/ \
    LIBCLANG_STATIC_PATH=/opt/rh/llvm-toolset-7/root/usr/lib64/ \
    CLANG_PATH=/opt/rh/llvm-toolset-7/root/usr/bin/clang
```

Build this image and use it, as is described extensively in [Custom Images](./custom_images.md).
