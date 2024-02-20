<!--toc:start-->
- [Configuring cross with environment variables](#configuring-cross-with-environment-variables)
- [Environment-Variable passthrough](#environment-variable-passthrough)
<!--toc:end-->

# Configuring cross with environment variables

Cross can be further customized by setting certain environment variables.
In-depth documentation with examples can be found [here][env-examples].

- `CROSS_CONTAINER_ENGINE`: The container engine to run cross in. Defaults to
  `docker` then `podman`, whichever is found first (example: `docker`, see the
  [FAQ][faq-container-engines]).
- `XARGO_HOME`: Home for [`xargo`][xargo-project] (example: `~/.xargo`).
- `NIX_STORE`: The directory for the [Nix store][nix-store] (example:
  `/nix/store`).
- `CROSS_CONTAINER_UID`: Set the user identifier for the cross command
  (example: `1000`).
- `CROSS_CONTAINER_GID`: Set the group identifier for the cross command
  (example: `1000`).
- `CROSS_CONTAINER_IN_CONTAINER`: Inform `cross` that it is running inside a
  container (example: `true`, see the FAQ).
- `CROSS_CONTAINER_OPTS`: Additional arguments to provide to the container
  engine during `$engine run` (example: `--env MYVAR=1` where `engine=docker`).
- `CROSS_CONFIG`: Specify the path to the `cross` config file (see [Config
  File][cross-config-file]).
- `CROSS_BUILD_OPTS`: Space separated flags to add when building a custom
  image, i.e. `--network=host`
- `CROSS_DEBUG`: Print debugging information for `cross`.
- `CROSS_COMPATIBILITY_VERSION`: Use older `cross` behavior (example: `0.2.1`).
- `CROSS_CUSTOM_TOOLCHAIN`: Specify that `rustup` is using a custom toolchain,
  and therefore should not try to add targets/install components. Useful with
  [`cargo-bisect-rustc`][cargo-bisect-rustc].
- `CROSS_REMOTE`: Inform `cross` it is using a remote container engine, and use
  data volumes rather than local bind mounts. See [Remote][docs-remote] for
  more information using remote container engines.
- `QEMU_STRACE`: Get a backtrace of system calls from “foreign” (non x86_64)
  binaries when using `cross` run.
- `CARGO_BUILD_TARGET`: Sets the default target, similar to specifying
  `--target`.
- `CROSS_ROOTLESS_CONTAINER_ENGINE`: Specify whether to container engine runs
  as root or is rootless. If set to `auto` or not provided, it assumes `docker`
  runs as root and all other container engines are rootless.
- `CROSS_CONTAINER_USER_NAMESPACE`: Custom the [container user
  namespace][container-user-namespace]. If set to `none`, user namespaces will
  be disabled. If not provided or set to `auto`, it will use the default
  namespace.
- `CROSS_CUSTOM_TOOLCHAIN_COMPAT`: A descriptive name for a custom toolchain so
  `cross` can convert it to a fully-qualified toolchain name.
- `CROSS_CONTAINER_ENGINE_NO_BUILDKIT`: The container engine does not have
  `buildx` command (or BuildKit support) when building custom images.
- `CROSS_NO_WARNINGS`: Set to `1` to panic on warnings from `cross`, before
  building the executables.
  Use `0` to disable this behaviour.
  The no warnings behaviour is implicitly enabled in CI pipelines.

All config file options can also be specified using environment variables. For
example, setting `CROSS_BUILD_XARGO=1` is identical to setting `build.xargo =
true`, and `CROSS_TARGET_AARCH64_UNKNOWN_LINUX_GNU_XARGO=1` is identical to
`target.aarch64-unknown-linux-gnu.xargo = true`.


# Environment-Variable passthrough

By default, `cross` does not pass most environment variables into the build
environment from the calling shell. This is chosen as a safe default as most
use cases will not want the calling environment leaking into the inner
execution environment. There are, however, some notable exceptions: most
environment variables `cross` or `cargo` reads are passed through automatically
to the build environment. The major exceptions are variables that are set by
`cross` or conflict with our build environment, including:

- `CARGO_HOME`
- `CARGO_TARGET_DIR`
- `CARGO_BUILD_TARGET_DIR`
- `CARGO_BUILD_RUSTC`
- `CARGO_BUILD_RUSTC_WRAPPER`
- `CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER`
- `CARGO_BUILD_RUSTDOC`
- `CROSS_RUNNER`
- `CROSS_RUSTC_MAJOR_VERSION`
- `CROSS_RUSTC_MINOR_VERSION`
- `CROSS_RUSTC_PATCH_VERSION`

Otherwise, any environment variables that start with CARGO_ or CROSS_, and a
few others, will be available in the build environment. For example, RUSTFLAGS
and CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS will both be automatically
available in the build environment.

In the instances that you do want to pass through additional environment
variables, this can be done via `build.env.passthrough` in your `Cross.toml`:

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


[env-examples]: https://github.com/cross-rs/wiki_assets/blob/main/Configuration/crossrc.bash_aliases
[faq-container-engines]: https://github.com/cross-rs/cross/wiki/FAQ#explicitly-choose-the-container-engine
[xargo-project]: https://github.com/japaric/xargo
[nix-store]: https://nixos.org/manual/nix/stable/introduction.html
[cross-config-file]: ./config_file.md
[cargo-bisect-rustc]: https://github.com/rust-lang/cargo-bisect-rustc
[docs-remote]: ./remote.md
[container-user-namespace]: https://docs.docker.com/engine/security/userns-remap/
