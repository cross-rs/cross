**Table Of Contents**

- [Getting Started](#getting-started)
  - [Dependencies](#dependencies)
  - [Building a Docker Image](#building-a-docker-image)
- [How Cross Works](#how-cross-works)
- [Design & Project Layout](#design-project-layout)
  - [Dockerfiles & Building Images](#dockerfiles-building-images)
  - [xtask](#xtask)
  - [CI Pipeline](#ci-pipeline)
- [Finding Easy Issues](#finding-easy-issues)
- [Releases and Versioning](#releases-and-versioning)
- [Code of Conduct](#code-of-conduct)

# Getting Started

`cross` is a tool to simplify cross-compiling Rust crates with minimal setup, by working as a drop-in replacement for `cargo`. To cross-compile for the target, `cross` first installs the target for your current Rust toolchain, mounts the package and Rust installation in a container with pre-installed tools for cross-compilation, and runs `cargo` inside the container.

Development on cross is broken down into 3 parts:
- Dockerfiles containing toolchains to cross-compile for a given target.
- The `cross` command.
- Peripherals to build and tag Docker images, and list or remove data (such as images) that cross stores on your machine.

If you plan to contribute code to `cross`, install the Git hooks to ensure your code passes lint and formatting checks prior to committing:

```bash
$ cargo xtask install-git-hooks
```

## Dependencies

Development on cross requires the following dependencies:
- Rust installed via [rustup](https://rustup.rs/).
- [Docker](https://docs.docker.com/engine/install/) or [Podman](https://podman.io/getting-started/installation.html).
- Git

## Building a Docker Image

Let's start off by building an image to cross-compile for a Linux OS running on IBM z/Architecture mainframes, or the `s390x-unknown-linux-gnu` target. We can build the target with:

```bash
$ cargo build-docker-image s390x-unknown-linux-gnu
```

This will build the image and tag it as `ghcr.io/cross-rs/s390x-unknown-linux-gnu:local`. More detailed information is provided in [Dockerfiles & Building Images](#dockerfiles-building-images) below.

# How Cross Works

<!--
TODO: in depth documentation on how rust finds the toolchain, finds 
the project root, mounts everything into a container, finds the 
required cross-compiler toolchains, cross-compiles the code and runs 
it.
-->

# Design & Project Layout

<!--
TODO: discuss the API limitations of cross, why cross-util and 
xtask exist, etc. describe how the project is structured, and 
each subcomponent.
-->

## Dockerfiles & Building Images

<!--
TODO: add documentation on creating and building docker images. 
This needs to describe the environment variables, such as 
`CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_LINKER` and 
`CC_s390x_unknown_linux_gnu` that are defined. 
It also needs to describe how to design and extend 
crosstool-ng-based images.

This also needs to probably describe the more complex installers
in depth, specifically: 
- `linux-image.sh`
- `android-ndk.sh`
- `android-system.sh`
- `crosstool-ng.sh`
- `musl.sh`
- `mingw.sh`
- `freebsd.sh`
- `dragonfly.sh`

Stuff that are trivial don't need to be documented:
- `aarch-linux-musl-gcc.sh`
- `dropbear.sh`
- `wine.sh`
- `xargo.sh`
-->

## xtask

`xtask` handles everything else related to the maintenance and development of the project.

<!-- TODO: describe build-docker-images and extract-target-info -->

## CI Pipeline

<!-- TODO: describe Github actions and how the pipeline works -->

# Finding Easy Issues

<!-- TODO: describe tags and how to find easier issues -->

# Releases and Versioning

`cross` follows [semantic versioning](https://semver.org/). For `cross` and `cross-util`, their command-line interface (CLI) is the public API: the library itself is an implementation-detail only. For the configuration files and environment variables, the removal of any options is a breaking change.

Certain details of the Dockerfiles are also part of the public API:
- Base distro (Ubuntu)
- libc

Incrementing the base distro version is always a breaking change, and updating the libc version is a breaking change only if it breaks compatibility with the distro it's based on. For example, upgrading the glibc version of `arm-unknown-linux-gnueabihf` from 2.17 to 2.23 would not be considered a breaking change, since it does not affect any compatibility with Debian packages, but upgrading the musllibc version of `unknown-linux-musleabihf` from 1.1.24 to 1.2.0 is because it changes [time_t](https://musl.libc.org/time64.html) from 32 to 64-bits.

Other tools, such as the runner (Qemu, WINE) can be incremented to a new major version without a breaking change unless major backwards-compatibility issues exist.

Since `cross` is meant to be used as a binary, and once installed is compatible with any version of `cargo`, changing the minimum required Rust version is not a breaking change.

<!-- 
@Emilgardis, is this right? 
TODO: more detail on releases 
-->

# Code of Conduct

Contribution to this crate is organized under the terms of the [Rust Code of
Conduct][CoC], the maintainer of this crate, the [cross-rs] team, promises
to intervene to uphold that code of conduct.

[CoC]: CODE_OF_CONDUCT.md
