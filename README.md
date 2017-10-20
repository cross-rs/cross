[![crates.io](https://img.shields.io/crates/v/cross.svg)](https://crates.io/crates/cross)
[![crates.io](https://img.shields.io/crates/d/cross.svg)](https://crates.io/crates/cross)

# `cross`

> "Zero setup" cross compilation and "cross testing" of Rust crates

<p align="center">
<img
  alt="`cross test`ing a crate for the aarch64-unknown-linux-gnu target"
  src="assets/cross-test.png"
  title="`cross test`ing a crate for the aarch64-unknown-linux-gnu target"
>
<br>
<em>`cross test`ing a crate for the aarch64-unknown-linux-gnu target</em>
</p>

**Disclaimer**: Only works on a x86_64 Linux host (e.g. Travis CI is supported)

## Features

- `cross` will provide all the ingredients needed for cross compilation without
  touching your system installation.

- `cross` provides an environment, cross toolchain and cross compiled libraries
  (e.g. OpenSSL), that produces the most portable binaries.

- "cross testing", `cross` can test crates for architectures other than i686 and
  x86_64.

- The stable, beta and nightly channels are supported.

## Dependencies

- [rustup](https://rustup.rs/)

- [Docker](https://www.docker.com/). Note that non-sudo users need to be in the
  `docker` group. Read the official [post-installation steps for Linux][post].

[post]: https://docs.docker.com/engine/installation/linux/linux-postinstall/

- A Linux kernel with [binfmt_misc] support is required for cross testing.

[binfmt_misc]: https://www.kernel.org/doc/html/latest/admin-guide/binfmt-misc.html

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

The default Docker image that `cross` uses provides a C environment that tries
to cover the most common cross compilation cases. However, it can't cover every
single use case out there. When the default image is not enough, you can use the
`target.$TARGET.image` field in `Cross.toml` to use custom Docker image for a
specific target:

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
cross uses: `japaric/$TARGET:$VERSION` (where `$VERSION` is cross's version).
This way you won't have to figure out how to install a cross C toolchain in your
custom image. Example below:

``` Dockerfile
FROM japaric/aarch64-unknown-linux-gnu:v0.1.4

RUN dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install libfoo:arm64
```

```
$ docker build -t my/image:tag path/to/where/the/Dockerfile/resides
```

### Passing environment variables into the build environemnt

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

### Use Xargo instead of Cargo

By default, `cross` uses `cargo` to build your Cargo project *unless* you are
building for one of the `thumbv*-none-eabi*` targets; in that case, it uses
`xargo`. However, you can use the `build.xargo` or `target.$TARGET.xargo` field
in `Cross.toml` to force the use of `xargo`:

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

Note that `xargo = false` has no effect as you can't use `cargo` with targets
that only support `xargo`.

## Supported targets

A target is considered as "supported" if `cross` can cross compile a
"non-trivial" (binary) crate, usually Cargo, for that target.

Testing support is more complicated. It relies on QEMU user emulation, so
testing may sometimes fail due to QEMU bug sand not because there's a bug in the
crate. That being said, `cross test` is assumed to "work" (`test` column in the
table below) if it can successfully
run [compiler-builtins](https://github.com/rust-lang-nursery/compiler-builtins)
test suite.

Also, testing is very slow. `cross` will actually run units tests *sequentially*
because QEMU gets upset when you spawn several threads. This also means that, if
one of your unit tests spawns several threads then it's more likely to fail or,
worst, "hang" (never terminate).

| Target                               |  libc  |   GCC   | OpenSSL | C++ | QEMU  | `test` |
|--------------------------------------|--------|---------|---------|:---:|-------|:------:|
| `aarch64-linux-android`              | N/A    | 4.9     | 1.0.2k  | ✓   | N/A   |        |
| `aarch64-unknown-linux-gnu`          | 2.19   | 4.8.2   | 1.0.2k  | ✓   | 2.8.0 |   ✓    |
| `arm-linux-androideabi`              | N/A    | 4.9     | 1.0.2k  | ✓   | N/A   |        |
| `arm-unknown-linux-gnueabi`          | 2.19   | 4.8.2   | 1.0.2k  | ✓   | 2.8.0 |   ✓    |
| `arm-unknown-linux-musleabi`         | 1.1.15 | 5.3.1   | N/A     |     | 2.8.0 |   ✓    |
| `armel-unknown-linux-gnueabi` [5]    | 2.19   | 4.9.2   | 1.0.2k  | ✓   | 2.9.0 |   ✓    |
| `armv7-linux-androideabi`            | N/A    | 4.9     | 1.0.2k  | ✓   | N/A   |        |
| `armv7-unknown-linux-gnueabihf`      | 2.15   | 4.6.2   | 1.0.2k  | ✓   | 2.8.0 |   ✓    |
| `armv7-unknown-linux-musleabihf`     | 1.1.15 | 5.3.1   | N/A     |     | 2.8.0 |   ✓    |
| `asmjs-unknown-emscripten` [4]       | 1.1.15 | 1.37.13 | N/A     | ✓   | N/A   |   ✓    |
| `i686-linux-android`                 | N/A    | 4.9     | 1.0.2k  | ✓   | N/A   |        |
| `i686-pc-windows-gnu`                | N/A    | 6.2.0   | N/A     | ✓   | N/A   |   ✓    |
| `i686-unknown-freebsd` [1]           | 10.2   | 5.3.0   | 1.0.2k  |     | N/A   |        |
| `i686-unknown-linux-gnu`             | 2.15   | 4.6.2   | 1.0.2k  | ✓   | N/A   |   ✓    |
| `i686-unknown-linux-musl`            | 1.1.15 | 5.3.1   | 1.0.2k  |     | N/A   |   ✓    |
| `mips-unknown-linux-gnu`             | 2.23   | 5.3.1   | 1.0.2k  | ✓   | 2.8.0 |   ✓    |
| `mips64-unknown-linux-gnuabi64`      | 2.23   | 5.3.1   | 1.0.2k  | ✓   | 2.8.0 |   ✓    |
| `mips64el-unknown-linux-gnuabi64`    | 2.23   | 5.3.1   | 1.0.2k  | ✓   | 2.8.0 |   ✓    |
| `mipsel-unknown-linux-gnu`           | 2.23   | 5.3.1   | 1.0.2k  | ✓   | 2.8.0 |   ✓    |
| `powerpc-unknown-linux-gnu`          | 2.19   | 4.8.2   | 1.0.2k  | ✓   | 2.7.1 |   ✓    |
| `powerpc64-unknown-linux-gnu`        | 2.19   | 4.8.2   | 1.0.2k  | ✓   | 2.7.1 |   ✓    |
| `powerpc64le-unknown-linux-gnu`      | 2.19   | 4.8.2   | 1.0.2k  | ✓   | 2.7.1 |   ✓    |
| `s390x-unknown-linux-gnu`            | 2.23   | 5.3.1   | 1.0.2k  | ✓   | 2.8.0 |        |
| `sparc64-unknown-linux-gnu` [2]      | 2.23   | 5.3.1   | 1.0.2k  | ✓   | 2.8.0 |   ✓    |
| `sparcv9-sun-solaris` [1]            | 2.11   | 5.3.0   | 1.0.2k  |     | N/A   |        |
| `thumbv6m-none-eabi` [3]             | 2.2.0  | 5.3.1   | N/A     |     | N/A   |        |
| `thumbv7em-none-eabi` [3]            | 2.2.0  | 5.3.1   | N/A     |     | N/A   |        |
| `thumbv7em-none-eabihf` [3]          | 2.2.0  | 5.3.1   | N/A     |     | N/A   |        |
| `thumbv7m-none-eabi` [3]             | 2.2.0  | 5.3.1   | N/A     |     | N/A   |        |
| `wasm32-unknown-emscripten` [4]      | 1.1.15 | 1.37.13 | N/A     | ✓   | N/A   |   ✓    |
| `x86_64-linux-android`               | N/A    | 4.9     | 1.0.2k  | ✓   | N/A   |        |
| `x86_64-pc-windows-gnu`              | N/A    | 6.2.0   | N/A     | ✓   | N/A   |   ✓    |
| `x86_64-sun-solaris` [1]             | 2.11   | 5.3.0   | 1.0.2k  |     | N/A   |        |
| `x86_64-unknown-dragonfly` [1] [2]   | 4.6.0  | 5.3.0   | 1.0.2k  |     | N/A   |   ✓    |
| `x86_64-unknown-freebsd` [1]         | 10.2   | 5.3.0   | 1.0.2k  |     | N/A   |        |
| `x86_64-unknown-linux-gnu`           | 2.15   | 4.6.2   | 1.0.2k  | ✓   | N/A   |   ✓    |
| `x86_64-unknown-linux-musl`          | 1.1.15 | 5.3.1   | 1.0.2k  |     | N/A   |   ✓    |
| `x86_64-unknown-netbsd`[1]           | 7.0    | 5.3.0   | 1.0.2k  |     | N/A   |        |

[1] For *BSD and Solaris targets, the libc column indicates the OS release version from
where libc was extracted.

[2] No `std` component available as of 2017-01-10

[3] libc = newlib

[4] libc = musl, gcc = emcc; Some projects that use libc may fail due to wrong
    definitions (will be fixed by https://github.com/rust-lang/libc/pull/610)

[5] armel = armv5te; See [Target Notes](./TARGET-NOTES.md#armv5) for additional info

## Debugging

### QEMU_STRACE (v0.1.9+)

You can set the QEMU_STRACE variable when you use `cross run` to get a backtrace
of system calls from "foreign" (non x86_64) binaries.

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

## Caveats / gotchas

- path dependencies (in Cargo.toml) that point outside the Cargo project won't
  work because `cross` use docker containers only mounts the Cargo project so
  the container doesn't have access to the rest of the filesystem.

- `cross` will mount the Cargo project as READ ONLY. Thus, if any crate attempts
  to modify its "source", the build will fail. Well behaved crates should only
  ever write to `$OUT_DIR` and never modify `$CARGO_MANIFEST_DIR` though.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
