# Change Log

All notable changes to this project will be documented in this file. This is an automatically-generated document: entries are added via changesets present in the `.changes` directory.
This project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [v0.2.5] - 2023-02-04

## Fixed

- #962 - fix SELinux labels to allow use in multiple containers and/or the host filesystem.
- #1166 - freebsd: include memstat in build image to fix build with libc 0.2.138 and up.
- #1183 - resolve issue when using `pre-build` in `Cargo.toml`

## [v0.2.4] - 2022-07-10

### Fixed

- #930 - fix any parsing of 1-character subcommands
- #929 - Fixed issue where `--verbose` would not output data when it should

## [v0.2.3] - 2022-07-09

### Added

- #921 - use `CARGO_TERM_VERBOSE`, `CARGO_TERM_QUIET`, and `CARGO_TERM_COLOR` environment variables for cross terminal output.
- #913 - added the `x86_64-unknown-illumos` target.
- #910 - `pre-build` can now take a string pointing to a script file to run.
- #905 - added `qemu-runner` for musl images, allowing use of native or emulated runners.
- #905 - added qemu emulation to `i586-unknown-linux-gnu`, `i686-unknown-linux-musl`, and `i586-unknown-linux-gnu`, so they can run on an `x86` CPU, rather than an `x86_64` CPU.
- #900 - add the option to skip copying build artifacts back to host when using remote cross via `CROSS_REMOTE_SKIP_BUILD_ARTIFACTS`.
- #891 - support custom user namespace overrides by setting the `CROSS_CONTAINER_USER_NAMESPACE` environment variable.
- #890 - support rootless docker via the `CROSS_ROOTLESS_CONTAINER_ENGINE` environment variable.
- #878 - added an image `ghcr.io/cross-rs/cross` containing cross.

### Changed

- #869 - ensure cargo configuration environment variable flags are passed to the docker container.
- #859 - added color diagnostic output and error messages.

### Fixed

- #905 - fixed running dynamically-linked libraries for all musl targets except `x86_64-unknown-linux-musl`.
- #904 - ensure `cargo metadata` works by using the same channel.
- #904 - fixed the path for workspace volumes and passthrough volumes with docker-in-docker.
- #898 - fix the path to the mount root with docker-in-docker if mounting volumes.
- #897 - ensure `target.$(...)` config options override `build` ones when parsing strings and vecs.
- #895 - convert filenames in docker tags to ASCII lowercase and ignore invalid characters
- #885 - handle symlinks when using remote docker.
- #868 - ignore the `CARGO` environment variable.
- #867 - fixed parsing of `build.env.passthrough` config values.

## [v0.2.2] - 2022-06-24

### Added

- #803 - added `CROSS_CUSTOM_TOOLCHAIN` to disable automatic installation of components for use with tools like `cargo-bisect-rustc`
- #795 - added images for additional toolchains maintained by cross-rs.
- #792 - added `CROSS_CONTAINER_IN_CONTAINER` environment variable to replace `CROSS_DOCKER_IN_DOCKER`.
- #785 - added support for remote container engines through data volumes through setting the `CROSS_REMOTE` environment variable. also adds in utility commands to create and remove persistent data volumes.
- #782 - added `build-std` config option, which builds the rust standard library from source if enabled.
- #678 - Add optional `target.{target}.dockerfile[.file]`, `target.{target}.dockerfile.context` and `target.{target}.dockerfile.build-args` to invoke docker/podman build before using an image.
- #678 - Add `target.{target}.pre-build` config for running commands before building the image.
- #772 - added `CROSS_CONTAINER_OPTS` environment variable to replace `DOCKER_OPTS`.
- #767, #788 - added the `cross-util` and `xtask` commands.
- #842 - Add `Cargo.toml` as configuration source
- #745 - added `thumbv7neon-*` targets.
- #741 - added `armv7-unknown-linux-gnueabi` and `armv7-unknown-linux-musleabi` targets.
- #721 - add support for running doctests on nightly if `CROSS_UNSTABLE_ENABLE_DOCTESTS=true`.
- #719 - add `--list` to known subcommands.
- #681 - Warn on unknown fields and confusable targets
- #624 - Add `build.default-target`
- #647 - Add `mips64-unknown-linux-muslabi64` and `mips64el-unknown-linux-muslabi64` support
- #543 - Added environment variables to control the UID and GID in the container
- #524 - docker: Add Nix Store volume support
- Added support for mounting volumes.
- #684 - Enable cargo workspaces to work from any path in the workspace, and make path dependencies mount seamlessly. Also added support for private SSH dependencies.

### Changed

- #838 - re-enabled the solaris targets.
- #807 - update Qemu to 6.1.0 on images using Ubuntu 18.04+ with python3.6+.
- #775 - forward Cargo exit code to host
- #762 - re-enabled `x86_64-unknown-dragonfly` target.
- #747 - reduced android image sizes.
- #746 - limit image permissions for android images.
- #377 - update WINE versions to 7.0.
- #734 - patch `arm-unknown-linux-gnueabihf` to build for ARMv6, and add architecture for crosstool-ng-based images.
- #709 - Update Emscripten targets to `emcc` version 3.1.10
- #707, #708 - Set `BINDGEN_EXTRA_CLANG_ARGS` environment variable to pass sysroot to `rust-bindgen`
- #696 - bump freebsd to 12.3
- #629 - Update Android NDK version and API version
- #497 - don't set RUSTFLAGS in aarch64-musl image
- #492 - Add cmake to FreeBSD images
- #748 - allow definitions in the environment variable passthrough

### Fixed

- #836 - write a `CACHEDIR.TAG` when creating the target directory, similar to `cargo`.
- #804 - allow usage of env `CARGO_BUILD_TARGET` as an alias for `CROSS_BUILD_TARGET`
- #792 - fixed container-in-container support when using podman.
- #781 - ensure `target.$(...)` config options override `build` ones.
- #771 - fix parsing of `DOCKER_OPTS`.
- #727 - add `PKG_CONFIG_PATH` to all `*-linux-gnu` images.
- #722 - boolean environment variables are evaluated as truthy or falsey.
- #720 - add android runner to preload `libc++_shared.so`.
- #725 - support `CROSS_DEBUG` and `CROSS_RUNNER` on android images.
- #714 - use host target directory when falling back to host cargo.
- #713 - convert relative target directories to absolute paths.
- #501 (reverted, see #764) - x86_64-linux: lower glibc version requirement to 2.17 (compatible with centos 7)
- #500 - use runner setting specified in Cross.toml
- #498 - bump linux-image version to fix CI
- Re-enabled `powerpc64-unknown-linux-gnu` image
- Re-enabled `sparc64-unknown-linux-gnu` image
- #582 - Added `libprocstat.so` to FreeBSD images
- #665 - when not using [env.volumes](https://github.com/cross-rs/cross#mounting-volumes-into-the-build-environment), mount project in /project
- #494 - Parse Cargo's --manifest-path option to determine mounted docker root

### Removed

- #718 - remove deb subcommand.

### Internal

- #856 - remove use of external wslpath and create internal helper that properly handles UNC paths.
- #828 - assume paths are Unicode and provide better error messages for path encoding errors.
- #787 - add installer for git hooks.
- #786, #791 - Migrate build script to rust: `cargo build-docker-image $TARGET`
- #730 - make FreeBSD builds more resilient.
- #670 - Use serde for deserialization of Cross.toml
- Change rust edition to 2021 and bump MSRV for the cross binary to 1.58.1
- #654 - Use color-eyre for error reporting
- #658 - Upgrade dependencies
- #652 - Allow trying individual targets via bors.
- #650 - Improve Docker caching.
- #609 - Switch to Github Actions and GHCR.
- #588 - fix ci: bump openssl version in freebsd again
- #552 - Added CHANGELOG.md automation
- #534 - fix image builds with update of dependencies
- #502 - fix ci: bump openssl version in freebsd
- #489 - Add support for more hosts and simplify/unify host support checks
- #477 - Fix Docker/Podman links in README
- #476 - Use Rustlang mirror for Sabotage linux tarballs
- Bump nix dependency to `0.22.1`
- Bump musl version to 1.1.24.

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

- Images are now hosted at <https://hub.docker.com/r/rustembedded/cross>.
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

- Partial `test` / `run` support for android targets. Using the android API via `cross run` / `cross test` is _not_ supported because Cross is using QEMU instead of the official Android emulator.

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
  on the host saving a _privileged_ docker run which some systems don't allow.

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

- Before Docker is invoked, `cross` will _always_ (re)generate the lockfile to
  avoid errors later on due to read/write permissions. This removes the need to
  call `cargo generate-lockfile` before `cross` in _all_ cases.

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

<!-- prettier-ignore-start -->
<!-- next-url -->

[Unreleased]: https://github.com/cross-rs/cross/compare/v0.2.5...HEAD

[v0.2.5]: https://github.com/cross-rs/cross/compare/v0.2.4...v0.2.5

[v0.2.4]: https://github.com/cross-rs/cross/compare/v0.2.3...v0.2.4

[v0.2.3]: https://github.com/cross-rs/cross/compare/v0.2.2...v0.2.3

[v0.2.2]: https://github.com/cross-rs/cross/compare/v0.2.1...v0.2.2
[v0.2.1]: https://github.com/cross-rs/cross/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/cross-rs/cross/compare/v0.1.16...v0.2.0
[v0.1.16]: https://github.com/cross-rs/cross/compare/v0.1.15...v0.1.16
[v0.1.15]: https://github.com/cross-rs/cross/compare/v0.1.14...v0.1.15
[v0.1.14]: https://github.com/cross-rs/cross/compare/v0.1.13...v0.1.14
[v0.1.13]: https://github.com/cross-rs/cross/compare/v0.1.12...v0.1.13
[v0.1.12]: https://github.com/cross-rs/cross/compare/v0.1.11...v0.1.12
[v0.1.11]: https://github.com/cross-rs/cross/compare/v0.1.10...v0.1.11
[v0.1.10]: https://github.com/cross-rs/cross/compare/v0.1.9...v0.1.10
[v0.1.9]: https://github.com/cross-rs/cross/compare/v0.1.8...v0.1.9
[v0.1.8]: https://github.com/cross-rs/cross/compare/v0.1.7...v0.1.8
[v0.1.7]: https://github.com/cross-rs/cross/compare/v0.1.6...v0.1.7
[v0.1.6]: https://github.com/cross-rs/cross/compare/v0.1.5...v0.1.6
[v0.1.5]: https://github.com/cross-rs/cross/compare/v0.1.4...v0.1.5
[v0.1.4]: https://github.com/cross-rs/cross/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/cross-rs/cross/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/cross-rs/cross/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/cross-rs/cross/compare/v0.1.0...v0.1.1
<!-- prettier-ignore-end -->
