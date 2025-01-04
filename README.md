[![crates.io](https://img.shields.io/crates/v/cross.svg)](https://crates.io/crates/cross)
[![crates.io](https://img.shields.io/crates/d/cross.svg)](https://crates.io/crates/cross)
[![CI](https://github.com/cross-rs/cross/actions/workflows/ci.yml/badge.svg?query=branch%3Amain)](https://github.com/cross-rs/cross/actions/workflows/ci.yml?query=branch)
[![Matrix](https://img.shields.io/matrix/cross-rs:matrix.org)](https://matrix.to/#/#cross-rs:matrix.org)

# `cross`

> “Zero setup” cross compilation and “cross testing” of Rust crates

This project is developed and maintained by the [cross-rs] team.
It was previously maintained by the Rust Embedded Working Group Tools team.
New contributors are welcome! Please join our [Matrix room] and say hi.

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

See our [Getting Started](./docs/getting-started.md) guide for detailed
installation instructions.

- [rustup](https://rustup.rs/)
- A Linux kernel with [binfmt_misc] support is required for cross testing.

One of these container engines is required. If both are installed, `cross` will
default to `docker`.

- [Docker]. Note that on Linux non-sudo users need to be in the `docker` group or use rootless docker.
  Read the container engine [install guide][docker_install] for the required installation and post-installation steps. Requires version 20.10 (API 1.40) or later.
- [Podman]. Requires version 3.4.0 or later.

## Installation

```sh
cargo install cross --git https://github.com/cross-rs/cross
```

It's also possible to directly download the pre-compiled [release
binaries](https://github.com/cross-rs/cross/releases) or use
[cargo-binstall](https://github.com/cargo-bins/cargo-binstall).

## Usage

`cross` has the exact same CLI as [Cargo](https://github.com/rust-lang/cargo)
but relies on Docker or Podman. For Docker, you'll have to start
the daemon before you can use it.

```
# (ONCE PER BOOT, on Linux)
# Start the Docker daemon, if it's not already running using systemd
# on WSL2 and other systems using SysVinit, use `sudo service docker start`.
$ sudo systemctl start docker

# MAGIC! This Just Works
$ cross build --target aarch64-unknown-linux-gnu

# EVEN MORE MAGICAL! This also Just Works
$ cross test --target mips64-unknown-linux-gnuabi64

# Obviously, this also Just Works
$ cross rustc --target powerpc-unknown-linux-gnu --release -- -C lto
```

Additional documentation can be found on the
[wiki](https://github.com/cross-rs/cross/wiki) or the `docs/` subfolder.

## Configuration

### Configuring cross behavior

You have four options to configure `cross`. All of these options use the TOML
format for configuration and the possible configuration values are documented
[here][config_file].

#### Option 1: Configuring `cross` directly in your `Cargo.toml`

You can directly set [configuration values][config_file] in your `Cargo.toml`
file, under the `[workspace.metadata.cross]` table, i.e. key prefix. An example
config snippet would look like this:

```toml,cargo
[workspace.metadata.cross.target.aarch64-unknown-linux-gnu]
# Install libssl-dev:arm64, see <https://github.com/cross-rs/cross/blob/main/docs/custom_images.md#adding-dependencies-to-existing-images>
pre-build = [
    "dpkg --add-architecture $CROSS_DEB_ARCH",
    "apt-get update && apt-get --assume-yes install libssl-dev:$CROSS_DEB_ARCH"
]
[workspace.metadata.cross.target.armv7-unknown-linux-gnueabi]
image = "my/image:latest"
[workspace.metadata.cross.build]
env.volumes = ["A_DIRECTORY=/path/to/volume"]
```

#### Option 2: Configuring `cross` via a `Cross.toml` file

You can put your [configuration][config_file] inside a `Cross.toml` file
in your project root directory.

#### Option 3: Using `CROSS_CONFIG` to specify the location of your configuration

By setting the `CROSS_CONFIG` environment variable, you can tell `cross` where
it should search for the config file. This way you are not limited to a
`Cross.toml` file in the project root.

#### Option 4: Configuring `cross` through environment variables

Besides the TOML-based configuration files, config can be passed through
[environment variables][docs_env_vars], too.


### Docker in Docker

When running `cross` from inside a container, `cross` needs access to
the hosts docker daemon itself. This is normally achieved by mounting the
docker daemons socket `/var/run/docker.sock`. For example:

```
$ docker run -v /var/run/docker.sock:/var/run/docker.sock -v .:/project \
  -w /project my/development-image:tag cross build --target mips64-unknown-linux-gnuabi64
```

The image running `cross` requires the rust development tools to be installed.

With this setup `cross` must find and mount the correct host paths into the
container used for cross compilation. This includes the original project
directory as well as the root path of the parent container to give access to
the rust build tools.

To inform `cross` that it is running inside a container set
`CROSS_CONTAINER_IN_CONTAINER=true`.

A development or CI container can be created like this:

```
FROM rust:1

# set CROSS_CONTAINER_IN_CONTAINER to inform `cross` that it is executed from within a container
ENV CROSS_CONTAINER_IN_CONTAINER=true

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

| Target                                 |  libc  |  GCC   | C++ | QEMU  | `test` |
|----------------------------------------|-------:|-------:|:---:|------:|:------:|
| `aarch64-linux-android` [1]            | 9.0.8  | 9.0.8  | ✓   | 6.1.0 |   ✓    |
| `aarch64-unknown-linux-gnu`            | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `aarch64-unknown-linux-gnu:centos` [7] | 2.17   | 4.8.5  |     | 4.2.1 |   ✓    |
| `aarch64-unknown-linux-musl`           | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `arm-linux-androideabi` [1]            | 9.0.8  | 9.0.8  | ✓   | 6.1.0 |   ✓    |
| `arm-unknown-linux-gnueabi`            | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `arm-unknown-linux-gnueabihf`          | 2.31   | 8.5.0  | ✓   | 6.1.0 |   ✓    |
| `arm-unknown-linux-musleabi`           | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `arm-unknown-linux-musleabihf`         | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `armv5te-unknown-linux-gnueabi`        | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `armv5te-unknown-linux-musleabi`       | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `armv7-linux-androideabi` [1]          | 9.0.8  | 9.0.8  | ✓   | 6.1.0 |   ✓    |
| `armv7-unknown-linux-gnueabi`          | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `armv7-unknown-linux-gnueabihf`        | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `armv7-unknown-linux-musleabi`         | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `armv7-unknown-linux-musleabihf`       | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `i586-unknown-linux-gnu`               | 2.31   | 9.4.0  | ✓   | N/A   |   ✓    |
| `i586-unknown-linux-musl`              | 1.2.3  | 9.2.0  | ✓   | N/A   |   ✓    |
| `i686-unknown-freebsd`                 | 1.5    | 6.4.0  | ✓   | N/A   |        |
| `i686-linux-android` [1]               | 9.0.8  | 9.0.8  | ✓   | 6.1.0 |   ✓    |
| `i686-pc-windows-gnu`                  | N/A    | 9.4    | ✓   | N/A   |   ✓    |
| `i686-unknown-linux-gnu`               | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `loongarch64-unknown-linux-gnu`        | 2.36   | 14.2.0 | ✓   | 8.2.2 |   ✓    |
| `loongarch64-unknown-linux-musl`       | 1.2.5  | 14.2.0 | ✓   | 8.2.2 |   ✓    |
| `mips-unknown-linux-gnu`               | 2.30   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `mips-unknown-linux-musl`              | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `mips64-unknown-linux-gnuabi64`        | 2.30   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `mips64-unknown-linux-muslabi64`       | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `mips64el-unknown-linux-gnuabi64`      | 2.30   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `mips64el-unknown-linux-muslabi64`     | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `mipsel-unknown-linux-gnu`             | 2.30   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `mipsel-unknown-linux-musl`            | 1.2.3  | 9.2.0  | ✓   | 6.1.0 |   ✓    |
| `powerpc-unknown-linux-gnu`            | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `powerpc64-unknown-linux-gnu`          | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `powerpc64le-unknown-linux-gnu`        | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `riscv64gc-unknown-linux-gnu`          | 2.35   | 11.4.0 | ✓   | 8.2.2 |   ✓    |
| `s390x-unknown-linux-gnu`              | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `sparc64-unknown-linux-gnu`            | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `sparcv9-sun-solaris`                  | 1.22.7 | 8.4.0  | ✓   | N/A   |        |
| `thumbv6m-none-eabi` [4]               | 3.3.0  | 9.2.1  |     | N/A   |        |
| `thumbv7em-none-eabi` [4]              | 3.3.0  | 9.2.1  |     | N/A   |        |
| `thumbv7em-none-eabihf` [4]            | 3.3.0  | 9.2.1  |     | N/A   |        |
| `thumbv7m-none-eabi` [4]               | 3.3.0  | 9.2.1  |     | N/A   |        |
| `thumbv7neon-linux-androideabi` [1]    | 9.0.8  | 9.0.8  | ✓   | 6.1.0 |   ✓    |
| `thumbv7neon-unknown-linux-gnueabihf`  | 2.31   | 9.4.0  | ✓   | N/A   |   ✓    |
| `thumbv8m.base-none-eabi` [4]          | 3.3.0  | 9.2.1  |     | N/A   |        |
| `thumbv8m.main-none-eabi` [4]          | 3.3.0  | 9.2.1  |     | N/A   |        |
| `thumbv8m.main-none-eabihf` [4]        | 3.3.0  | 9.2.1  |     | N/A   |        |
| `wasm32-unknown-emscripten` [6]        | 3.1.14 | 15.0.0 | ✓   | N/A   |   ✓    |
| `x86_64-linux-android` [1]             | 9.0.8  | 9.0.8  | ✓   | 6.1.0 |   ✓    |
| `x86_64-pc-windows-gnu`                | N/A    | 9.3    | ✓   | N/A   |   ✓    |
| `x86_64-pc-solaris`                    | 1.22.7 | 8.4.0  | ✓   | N/A   |        |
| `x86_64-unknown-freebsd`               | 1.5    | 6.4.0  | ✓   | N/A   |        |
| `x86_64-unknown-dragonfly` [2] [3]     | 6.0.1  | 10.3.0 | ✓   | N/A   |        |
| `x86_64-unknown-illumos`               | 1.20.4 | 8.4.0  | ✓   | N/A   |        |
| `x86_64-unknown-linux-gnu`             | 2.31   | 9.4.0  | ✓   | 6.1.0 |   ✓    |
| `x86_64-unknown-linux-gnu:centos` [5]  | 2.17   | 4.8.5  | ✓   | 4.2.1 |   ✓    |
| `x86_64-unknown-linux-musl`            | 1.2.3  | 9.2.0  | ✓   | N/A   |   ✓    |
| `x86_64-unknown-netbsd` [3]            | 9.2.0  | 9.4.0  | ✓   | N/A   |        |
<!--| `asmjs-unknown-emscripten` [7]       | 3.1.14 | 15.0.0  | ✓   | N/A   |   ✓    |-->

[1] libc = bionic; Only works with native tests, that is, tests that do not
    depends on the Android Runtime. For i686 some tests may fails with the
    error `assertion failed: signal(libc::SIGPIPE, libc::SIG_IGN) !=
    libc::SIG_ERR`, see [issue
    #140](https://github.com/cross-rs/cross/issues/140) for more information.

[2] No `std` component available.

[3] For some \*BSD and Solaris targets, the libc column indicates the OS
    release version from which libc was extracted.

[4] libc = newlib

[5] Must change
    `image = "ghcr.io/cross-rs/x86_64-unknown-linux-gnu:main-centos"` in
    `Cross.toml` for `[target.x86_64-unknown-linux-gnu]` to use the
    CentOS7-compatible target.

[6] libc = emscripten and GCC = clang

[7] Must change
    `image = "ghcr.io/cross-rs/aarch64-unknown-linux-gnu:main-centos"` in
    `Cross.toml` for `[target.aarch64-unknown-linux-gnu]` to use the
    CentOS7-compatible target.

<!--[7] libc = emscripten and GCC = clang. The Docker images for these targets are currently not built automatically
due to a [compiler bug](https://github.com/rust-lang/rust/issues/98216), you will have to build them yourself for now.-->

Additional Dockerfiles for other targets can be found in
[cross-toolchains](https://github.com/cross-rs/cross-toolchains). These include
MSVC and Apple Darwin targets, which we cannot ship pre-built images of.


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

## Minimum Supported Rust Version (MSRV)

This crate is guaranteed to compile on stable Rust 1.77.2 and up. It *might*
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
Conduct][CoC], the maintainer of this crate, the [cross-rs] team, promises
to intervene to uphold that code of conduct.

[CoC]: CODE_OF_CONDUCT.md
[cross-rs]: https://github.com/cross-rs
[Docker]: https://www.docker.com
[Podman]: https://podman.io
[Matrix room]: https://matrix.to/#/#cross-rs:matrix.org
[docker_install]: https://github.com/cross-rs/cross/wiki/Getting-Started#installing-a-container-engine
[binfmt_misc]: https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html
[config_file]: ./docs/config_file.md
[docs_env_vars]: ./docs/environment_variables.md
