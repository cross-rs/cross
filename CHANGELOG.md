# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).

## [Unreleased]

- #552 - Added CHANGELOG.md automation
- #543 - Added environment variables to control the UID and GID in the container
- #534 - fix image builds with update of dependencies
- #502 - fix ci: bump openssl version in freebsd
- #501 - x86_64-linux: lower glibc version requirement to 2.17 (compatible with centos 7) 
- #500 - use runner setting specified in Cross.toml 
- #498 - bump linux-image version to fix CI
- #497 - don't set RUSTFLAGS in aarch64-musl image 
- #492 - Add cmake to FreeBSD images
- #489 - Add support for more hosts and simplify/unify host support checks
- #477 - Fix Docker/Podman links in README
- #476 - Use Rustlang mirror for Sabotage linux tarbals
- Added support for mounting volumes.
- Re-enabled `powerpc64-unknown-linux-gnu` image
- Re-enabled `sparc64-unknown-linux-gnu` image

## [v0.2.1] - 2020-06-30

- Disabled `powerpc64-unknown-linux-gnu` image.
- Disabled `sparc64-unknown-linux-gnu` image.
- Disabled `x86_64-unknown-dragonfly` image.
- Removed CI testing for `i686-apple-darwin`.

## [v0.2.0] - 2020-02-22

- Removed OpenSSL from all images.
- Added support for Podman.
- Bumped all images to at least Ubuntu 16.04.

## [v0.1.16] - 2019-09-17

- Bump OpenSSL version to 1.0.2t.
- Re-enabled `asmjs-unknown-emscripten` target.
- Default to `native` runner instead of `qemu-user` for certain targets.

## [v0.1.15] - 2019-09-04

- Images are now hosted at https://hub.docker.com/r/rustembedded/cross.
- Bump OpenSSL version to 1.0.2p.
- Bump musl version to 1.1.20.
- Bump Ubuntu to 18.04 to all musl targets.
- Bump gcc version to 6.3.0 for all musl targets.
- OpenSSL support for the `arm-unknown-linux-musleabi` target.
- OpenSSL support for the `armv7-unknown-linux-musleabihf` target.
- Build and test support for `aarch64-unknown-linux-musl`, `arm-unknown-linux-musleabihf`,
  `armv5te-unknown-linux-musleabi`, `i586-unknown-linux-musl`, `mips-unknown-linux-musl`,
  add `mipsel-unknown-linux-musl` targets.

## [v0.1.14] - 2017-11-22

### Added

- Support for the `i586-unknown-linux-gnu` target.

### Changed

- Downgraded the Solaris toolchains from 2.11 to 2.10 to make the binaries produced by Cross more
  compatible (this version matches what rust-lang/rust is using).

## [v0.1.13] - 2017-11-08

### Added

- Support for the custom [`deb`] subcommand.

[`deb`]: https://github.com/mmstick/cargo-deb

- Partial `test` / `run` support for android targets. Using the android API via `cross run` / `cross
  test` is *not* supported because Cross is using QEMU instead of the official Android emulator.

- Partial support for the `sparcv9-sun-solaris` and `x86_64-sun-solaris` targets. `cross test` and
  `cross run` doesn't work for these new targets.

- OpenSSL support for the `i686-unknown-linux-musl` target.

### Changed

- Bump OpenSSL version to 1.0.2m.

## [v0.1.12] - 2017-09-22

### Added

- Support for `cross check`. This subcommand won't use any Docker container.

### Changed

- `binfmt_misc` is not required on the host for toolchain v1.19.0 and newer.
  With these toolchains `binfmt_misc` interpreters don't need to be installed
  on the host saving a *privileged* docker run which some systems don't allow.

## [v0.1.11] - 2017-06-10

### Added

- Build and test support for `i686-pc-windows-gnu`, `x86_64-pc-windows-gnu`,
  `asmjs-unknown-emscripten` and `wasm-unknown-emscripten`.

- Build support for `aarch64-linux-android`, `arm-linux-androideabi`,
  `armv7-linux-androideabi`, `x86_64-linux-android` and `i686-linux-android`

- A `build.env.passthrough` / `build.target.*.passthrough` option to Cross.toml
  to support passing environment variables from the host to the Docker image.

### Changed

- Bumped OpenSSL version to 1.0.2k
- Bumped QEMU version to 2.9.0

## [v0.1.10] - 2017-04-02

### Added

- Cross compilation support for `x86_64-pc-windows-gnu`

- Cross compilation support for Android targets

### Changed

- Bumped OpenSSL version to 1.0.2k

## [v0.1.9] - 2017-02-08

### Added

- Support for ARM MUSL targets.

### Changed

- The automatic lockfile update that happens every time `cross` is invoked
  should no longer hit the network when there's no git dependency to add/update.

- The QEMU_STRACE variable is passed to the underlying Docker container. Paired
  with `cross run`, this lets you get a trace of system call from the execution
  of "foreign" (non x86_64) binaries.

## [v0.1.8] - 2017-01-21

### Added

- Support for custom targets. Cross will now also try to use a docker image for
  them. As with the built-in targets, one can override the image using
  `[target.{}.image]` in Cross.toml.

### Changed

- Moved to a newer Xargo: v0.3.5

## [v0.1.7] - 2017-01-19

### Changed

- Moved to a newer Xargo: v0.3.4

### Fixed

- QEMU interpreters were being register when not required, e.g. for the
  `x86_64-unknown-linux-gnu` target.

## [v0.1.6] - 2017-01-14

### Fixed

- Stable releases were picking the wrong image (wrong tag: 0.1.5 instead of
  v0.1.5)

## [v0.1.5] - 2017-01-14 [YANKED]

### Added

- `cross run` support for the thumb targets.

- A `build.xargo` / `target.$TARGET.xargo` option to Cross.toml to use Xargo
  instead of Cargo.

- A `target.$TARGET.image` option to override the Docker image used for
  `$TARGET`.

- A `sparc64-unknown-linux-gnu` environment.

- A `x86_64-unknown-dragonfly` environment.

### Changed

- Building older versions (<0.7.0) of the `openssl` crate is now supported.

- Before Docker is invoked, `cross` will *always* (re)generate the lockfile to
  avoid errors later on due to read/write permissions. This removes the need to
  call `cargo generate-lockfile` before `cross` in *all* cases.

## [v0.1.4] - 2017-01-07

### Added

- Support for the `arm-unknown-linux-gnueabi` target

- `cross build` support for:
  - `i686-unknown-freebsd`
  - `x86_64-unknown-freebsd`
  - `x86_64-unknown-netbsd`

### Changed

- It's no longer necessary to call `cargo generate-lockfile` before using
  `cross` as `cross` will now take care of creating a lockfile when necessary.

- The C environments for the `thumb` targets now include newlib (`libc.a`,
  `libm.a`, etc.)

### Fixed

- A segfault when `cross` was trying to figure out the name of the user that
  called it.

## [v0.1.3] - 2017-01-01

### Changed

- Fix the `i686-unknown-linux-musl` target

## [v0.1.2] - 2016-12-31

### Added

- Support for `i686-unknown-linux-musl`
- Support for `cross build`ing crates for the `thumbv*-none-eabi*` targets.

## [v0.1.1] - 2016-12-28

### Added

- Support for `x86_64-unknown-linux-musl`
- Print shell commands when the verbose flag is used.
- Support crossing from x86_64 osx to i686 osx

## v0.1.0 - 2016-12-26

- Initial release. Supports 12 targets.

[Unreleased]: https://github.com/rust-embedded/cross/compare/v0.2.1...HEAD
[v0.2.1]: https://github.com/rust-embedded/cross/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/rust-embedded/cross/compare/v0.1.16...v0.2.0
[v0.1.16]: https://github.com/rust-embedded/cross/compare/v0.1.15...v0.1.16
[v0.1.15]: https://github.com/rust-embedded/cross/compare/v0.1.14...v0.1.15
[v0.1.14]: https://github.com/rust-embedded/cross/compare/v0.1.13...v0.1.14
[v0.1.13]: https://github.com/rust-embedded/cross/compare/v0.1.12...v0.1.13
[v0.1.12]: https://github.com/rust-embedded/cross/compare/v0.1.11...v0.1.12
[v0.1.11]: https://github.com/rust-embedded/cross/compare/v0.1.10...v0.1.11
[v0.1.10]: https://github.com/rust-embedded/cross/compare/v0.1.9...v0.1.10
[v0.1.9]: https://github.com/rust-embedded/cross/compare/v0.1.8...v0.1.9
[v0.1.8]: https://github.com/rust-embedded/cross/compare/v0.1.7...v0.1.8
[v0.1.7]: https://github.com/rust-embedded/cross/compare/v0.1.6...v0.1.7
[v0.1.6]: https://github.com/rust-embedded/cross/compare/v0.1.5...v0.1.6
[v0.1.5]: https://github.com/rust-embedded/cross/compare/v0.1.4...v0.1.5
[v0.1.4]: https://github.com/rust-embedded/cross/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/rust-embedded/cross/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/rust-embedded/cross/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/rust-embedded/cross/compare/v0.1.0...v0.1.1
