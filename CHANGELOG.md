# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).

## [Unreleased]

### Added

- Support for emscripten targets.

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

[Unreleased]: https://github.com/japaric/cross/compare/v0.1.9...HEAD
[v0.1.9]: https://github.com/japaric/cross/compare/v0.1.8...v0.1.9
[v0.1.8]: https://github.com/japaric/cross/compare/v0.1.7...v0.1.8
[v0.1.7]: https://github.com/japaric/cross/compare/v0.1.6...v0.1.7
[v0.1.6]: https://github.com/japaric/cross/compare/v0.1.5...v0.1.6
[v0.1.5]: https://github.com/japaric/cross/compare/v0.1.4...v0.1.5
[v0.1.4]: https://github.com/japaric/cross/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/japaric/cross/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/japaric/cross/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/japaric/cross/compare/v0.1.0...v0.1.1
