[![crates.io](https://img.shields.io/crates/v/cross.svg)](https://crates.io/crates/cross)
[![crates.io](https://img.shields.io/crates/d/cross.svg)](https://crates.io/crates/cross)
[![Build Status](https://img.shields.io/azure-devops/build/rust-embedded/c0ce10ee-fd41-4551-b3e3-9612e8ab62f3/2/master)](https://dev.azure.com/rust-embedded/cross/_build?definitionId=2)
[![Docker Pulls](https://img.shields.io/docker/pulls/rustembedded/cross)](https://hub.docker.com/r/rustembedded/cross)

# `cross`

> “Zero setup” cross compilation and “cross testing” of Rust crates

This project is developed and maintained by the [Tools team][team].

<p align="center">
<img
  alt="`cross test`ing a crate for the aarch64-unknown-linux-gnu target"
  src="assets/cross-test.png"
  title="`cross test`ing a crate for the aarch64-unknown-linux-gnu target"
>
<br>
<em>`cross test`ing a crate for the aarch64-unknown-linux-gnu target</em>
</p>

## Features

- `cross` will provide all the ingredients needed for cross compilation without
  touching your system installation.

- `cross` provides an environment, cross toolchain and cross compiled libraries,
  that produces the most portable binaries.

- “cross testing”, `cross` can test crates for architectures other than i686 and
  x86_64.

- The stable, beta and nightly channels are supported.

## Dependencies

- [rustup](https://rustup.rs/)

- A Linux kernel with [binfmt_misc] support is required for cross testing.

[binfmt_misc]: https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html

One of these container engines is required. If both are installed, `cross` will
default to `docker`.

- [Docker]. Note that on Linux non-sudo users need to be in the `docker` group.
  Read the official [post-installation steps][post]. Requires version 1.24 or later.

[post]: https://docs.docker.com/install/linux/linux-postinstall/

- [Podman]. Requires version 1.6.3 or later.

## Installation

```
$ cargo install cross
```

## Usage

`cross` has the exact same CLI as [Cargo](https://github.com/rust-lang/cargo)
but as it relies on Docker you'll have to start the daemon before you can use
it.

```
# (ONCE PER BOOT)
# Start the Docker daemon, if it's not already running
$ sudo systemctl start docker

# MAGIC! This Just Works
$ cross build --target aarch64-unknown-linux-gnu

# EVEN MORE MAGICAL! This also Just Works
$ cross test --target mips64-unknown-linux-gnuabi64

# Obviously, this also Just Works
$ cross rustc --target powerpc-unknown-linux-gnu --release -- -C lto
```

## Configuration

You can place a `Cross.toml` file in the root of your Cargo project to tweak
`cross`'s behavior:

### Custom Docker images

`cross` provides default Docker images for the targets listed below. However, it
can't cover every single use case out there. For other targets, or when the
default image is not enough, you can use the `target.{{TARGET}}.image` field in
`Cross.toml` to use custom Docker image for a specific target:

``` toml
[target.aarch64-unknown-linux-gnu]
image = "my/image:tag"
```

In the example above, `cross` will use a image named `my/image:tag` instead of
the default one. Normal Docker behavior applies, so:

- Docker will first look for a local image named `my/image:tag`

- If it doesn't find a local image, then it will look in Docker Hub.

- If only `image:tag` is specified, then Docker won't look in Docker Hub.

- If only `tag` is omitted, then Docker will use the `latest` tag.

It's recommended to base your custom image on the default Docker image that
cross uses: `rustembedded/cross:{{TARGET}}-{{VERSION}}` (where `{{VERSION}}` is cross's version).
This way you won't have to figure out how to install a cross C toolchain in your
custom image. Example below:

``` Dockerfile
FROM rustembedded/cross:aarch64-unknown-linux-gnu-0.2.1

RUN dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install --assume-yes libfoo:arm64
```

```
$ docker build -t my/image:tag path/to/where/the/Dockerfile/resides
```

### Docker in Docker

When running `cross` from inside a docker container, `cross` needs access to
the hosts docker daemon itself. This is normally achieved by mounting the
docker daemons socket `/var/run/docker.sock`. For example:

```
$ docker run -v /var/run/docker.sock:/var/run/docker.sock -v .:/project \
  -w /project my/development-image:tag cross build --target mips64-unknown-linux-gnuabi64
```

The image running `cross` requires the rust development tools to be installed.

With this setup `cross` must find and mount the correct host paths into the
container used for cross compilation. This includes the original project directory as
well as the root path of the parent container to give access to the rust build
tools.

To inform `cross` that it is running inside a container set `CROSS_DOCKER_IN_DOCKER=true`.

A development or CI container can be created like this:

```
FROM rust:1

# set CROSS_DOCKER_IN_DOCKER to inform `cross` that it is executed from within a container
ENV CROSS_DOCKER_IN_DOCKER=true

# install `cross`
RUN cargo install cross

...

```

**Limitations**: Finding the mount point for the containers root directory is
currently only available for the overlayfs2 storage driver. In order to access
the parent containers rust setup, the child container mounts the parents
overlayfs. The parent must not be stopped before the child container, as the
overlayfs can not be unmounted correctly by Docker if the child container still
accesses it.


### Explicitly choose the container engine

By default, `cross` tries to use [Docker] or [Podman], in that order.
If you want to choose a container engine explicitly, you can set the
binary name (or path) using the `CROSS_CONTAINER_ENGINE`
environment variable.

For example in case you want use [Podman], you can set `CROSS_CONTAINER_ENGINE=podman`.

### Passing environment variables into the build environment

By default, `cross` does not pass any environment variables into the build
environment from the calling shell. This is chosen as a safe default as most use
cases will not want the calling environment leaking into the inner execution
environment.

In the instances that you do want to pass through environment variables, this
can be done via `build.env.passthrough` in your `Cross.toml`:

```toml
[build.env]
passthrough = [
    "RUST_BACKTRACE",
    "RUST_LOG",
    "TRAVIS",
]
```

To pass variables through for one target but not others, you can use
this syntax instead:

```toml
[target.aarch64-unknown-linux-gnu.env]
passthrough = [
    "RUST_DEBUG",
]
```

### Mounting volumes into the build environment

In addition to passing environment variables, you can also specify environment
variables pointing to paths which should be mounted into the container:

```toml
[target.aarch64-unknown-linux-gnu.env]
volumes = [
    "BUILD_DIR",
]
```

### Use Xargo instead of Cargo

By default, `cross` uses `xargo` to build your Cargo project only for all
non-standard targets (i.e. something not reported by rustc/rustup). However,
you can use the `build.xargo` or `target.{{TARGET}}.xargo` field in
`Cross.toml` to force the use of `xargo`:

``` toml
# all the targets will use `xargo`
[build]
xargo = true
```

Or,

``` toml
# only this target will use `xargo`
[target.aarch64-unknown-linux-gnu]
xargo = true
```

`xargo = false` will work the opposite way (pick cargo always) and is useful
when building for custom targets that you know to work with cargo.

## Supported targets

A target is considered as “supported” if `cross` can cross compile a
“non-trivial” (binary) crate, usually Cargo, for that target.

Testing support (`cross test`) is more complicated. It relies on [QEMU]
emulation, so testing may fail due to QEMU bugs rather than bugs in your crate.
That said, a target has a ✓ in `test` column of the table below if it can run
the [`compiler-builtins`] test suite.

[QEMU]: https://www.qemu.org/
[`compiler-builtins`]: https://github.com/rust-lang-nursery/compiler-builtins

Also, testing is very slow. `cross test` runs units tests *sequentially* because
QEMU gets upset when you spawn multiple threads. This means that, if one of your
unit tests spawns threads, then it's more likely to fail or, worst, never
terminate.

| Target                               |  libc  |   GCC   | C++ | QEMU  | `test` |
|--------------------------------------|-------:|--------:|:---:|------:|:------:|
| `*-apple-ios` [1]                    | N/A    | N/A     | N/A | N/A   |   ✓    |
| `aarch64-linux-android` [2]          | N/A    | 4.9     | ✓   | N/A   |   ✓    |
| `aarch64-unknown-linux-gnu`          | 2.19   | 4.8.2   | ✓   | 4.1.0 |   ✓    |
| `aarch64-unknown-linux-musl`         | 1.1.24 | 6.3.0   |     | 4.1.0 |   ✓    |
| `arm-linux-androideabi` [2]          | N/A    | 4.9     | ✓   | N/A   |   ✓    |
| `arm-unknown-linux-gnueabi`          | 2.19   | 4.8.2   | ✓   | 4.1.0 |   ✓    |
| `arm-unknown-linux-gnueabihf`        | 2.27   | 7.3.0   | ✓   | 4.1.0 |   ✓    |
| `arm-unknown-linux-musleabi`         | 1.1.24 | 6.3.0   |     | 4.1.0 |   ✓    |
| `arm-unknown-linux-musleabihf`       | 1.1.24 | 6.3.0   |     | 4.1.0 |   ✓    |
| `armv5te-unknown-linux-gnueabi`      | 2.27   | 7.5.0   | ✓   | 4.2.0 |   ✓    |
| `armv5te-unknown-linux-musleabi`     | 1.1.24 | 6.3.0   |     | 4.1.0 |   ✓    |
| `armv7-linux-androideabi` [2]        | N/A    | 4.9     | ✓   | N/A   |   ✓    |
| `armv7-unknown-linux-gnueabihf`      | 2.15   | 4.6.2   | ✓   | 4.1.0 |   ✓    |
| `armv7-unknown-linux-musleabihf`     | 1.1.24 | 6.3.0   |     | 4.1.0 |   ✓    |
| `i586-unknown-linux-gnu`             | 2.23   | 5.3.1   | ✓   | N/A   |   ✓    |
| `i586-unknown-linux-musl`            | 1.1.24 | 6.3.0   |     | N/A   |   ✓    |
| `i686-unknown-freebsd` [4]           | 12.1   | 6.4.0   |     | N/A   |   ✓    |
| `i686-linux-android` [2]             | N/A    | 4.9     | ✓   | N/A   |   ✓    |
| `i686-pc-windows-gnu`                | N/A    | 7.3.0   | ✓   | N/A   |   ✓    |
| `i686-unknown-linux-gnu`             | 2.15   | 4.6.2   | ✓   | N/A   |   ✓    |
| `i686-unknown-linux-musl`            | 1.1.24 | 6.3.0   |     | N/A   |   ✓    |
| `mips-unknown-linux-gnu`             | 2.23   | 5.3.1   | ✓   | 4.1.0 |   ✓    |
| `mips-unknown-linux-musl`            | 1.1.24 | 6.3.0   | ✓   | 4.1.0 |   ✓    |
| `mips64-unknown-linux-gnuabi64`      | 2.23   | 5.3.1   | ✓   | 4.1.0 |   ✓    |
| `mips64el-unknown-linux-gnuabi64`    | 2.23   | 5.3.1   | ✓   | 4.1.0 |   ✓    |
| `mipsel-unknown-linux-gnu`           | 2.23   | 5.3.1   | ✓   | 4.1.0 |   ✓    |
| `mipsel-unknown-linux-musl`          | 1.1.24 | 6.3.0   | ✓   | 4.1.0 |   ✓    |
| `powerpc-unknown-linux-gnu`          | 2.19   | 4.8.2   | ✓   | 3.0.1 |   ✓    |
| `powerpc64-unknown-linux-gnu`        | 2.31   | 10.2.0  | ✓   | 3.0.1 |   ✓    |
| `powerpc64le-unknown-linux-gnu`      | 2.19   | 4.8.2   | ✓   | 3.0.1 |   ✓    |
| `riscv64gc-unknown-linux-gnu`        | 2.27   | 7.5.0   | ✓   | 4.2.0 |   ✓    |
| `s390x-unknown-linux-gnu`            | 2.23   | 5.3.1   | ✓   | 4.1.0 |        |
| `sparc64-unknown-linux-gnu`          | 2.31   | 10.2.0  | ✓   | 4.2.0 |   ✓    |
| `sparcv9-sun-solaris` [4]            | 2.11   | 5.3.0   | ✓   | N/A   |        |
| `thumbv6m-none-eabi` [5]             | 2.2.0  | 5.3.1   |     | N/A   |        |
| `thumbv7em-none-eabi` [5]            | 2.2.0  | 5.3.1   |     | N/A   |        |
| `thumbv7em-none-eabihf` [5]          | 2.2.0  | 5.3.1   |     | N/A   |        |
| `thumbv7m-none-eabi` [5]             | 2.2.0  | 5.3.1   |     | N/A   |        |
| `wasm32-unknown-emscripten` [6]      | 1.1.15 | 1.37.13 | ✓   | N/A   |   ✓    |
| `x86_64-linux-android` [2]           | N/A    | 4.9     | ✓   | N/A   |   ✓    |
| `x86_64-pc-windows-gnu`              | N/A    | 7.3.0   | ✓   | N/A   |   ✓    |
| `x86_64-sun-solaris` [4]             | 2.11   | 5.3.0   | ✓   | N/A   |        |
| `x86_64-unknown-freebsd` [4]         | 12.1   | 6.4.0   |     | N/A   |   ✓    |
| `x86_64-unknown-dragonfly` [4] [3]   | 4.6.0  | 5.3.0   | ✓   | N/A   |        |
| `x86_64-unknown-linux-gnu`           | 2.15   | 4.6.2   | ✓   | N/A   |   ✓    |
| `x86_64-unknown-linux-musl`          | 1.1.24 | 6.3.0   |     | N/A   |   ✓    |
| `x86_64-unknown-netbsd` [4]          | 7.0    | 5.3.0   | ✓   | N/A   |        |

[1] iOS cross compilation is supported on macOS hosts.

[2] Only works with native tests, that is, tests that do not depends on the
    Android Runtime. For i686 some tests may fails with the error `assertion
    failed: signal(libc::SIGPIPE, libc::SIG_IGN) != libc::SIG_ERR`, see
    [issue #140](https://github.com/rust-embedded/cross/issues/140) for more
    information.

[4] For *BSD and Solaris targets, the libc column indicates the OS release version
    from which libc was extracted.

[3] No `std` component available as of 2017-01-10

[5] libc = newlib

[6] libc = musl, gcc = emcc; Some projects that use libc may fail due to wrong
    definitions (will be fixed by https://github.com/rust-lang/libc/pull/610)

## Debugging

### QEMU_STRACE (v0.1.9+)

You can set the QEMU_STRACE variable when you use `cross run` to get a backtrace
of system calls from “foreign” (non x86_64) binaries.

```
$ cargo new --bin hello && cd $_

$ QEMU_STRACE=1 cross run --target aarch64-unknown-linux-gnu
9 brk(NULL) = 0x0000004000023000
9 uname(0x4000823128) = 0
(..)
9 write(1,0xa06320,14)Hello, world!
 = 14
9 sigaltstack(0x4000823588,(nil)) = 0
9 munmap(0x0000004000b16000,16384) = 0
9 exit_group(0)
```

## Caveats

- path dependencies (in Cargo.toml) that point outside the Cargo project won't
  work because `cross` use docker containers only mounts the Cargo project so
  the container doesn't have access to the rest of the filesystem.

## Minimum Supported Rust Version (MSRV)

This crate is guaranteed to compile on stable Rust 1.42.0 and up. It *might*
compile with older versions but that may change in any new patch release.

Some cross-compilation targets require a later Rust version, and using Xargo
requires a nightly Rust toolchain.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Code of Conduct

Contribution to this crate is organized under the terms of the [Rust Code of
Conduct][CoC], the maintainer of this crate, the [Tools team][team], promises
to intervene to uphold that code of conduct.

[CoC]: CODE_OF_CONDUCT.md
[team]: https://github.com/rust-embedded/wg#the-tools-team
[Docker]: https://www.docker.com
[Podman]: https://podman.io
