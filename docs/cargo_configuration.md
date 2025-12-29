<!--toc:start-->
- [Configuring `cross`](#configuring-cross)
- [Configuring Cargo through environment variables](#configuring-cargo-through-environment-variables)
<!--toc:end-->

# Configuring `cross`

Please refer to the following docs:

- [config file](./config_file.md)
- [env variables](./environment_variables.md)


# Configuring Cargo through environment variables

When cross-compiling, `cargo` does not use environment variables such as
`RUSTFLAGS`, and must be provided using `CARGO_TARGET_${TARGET}_${OPTION}`.
Please note that some of these may be provided by the image themselves, such as
runners, and should be overwritten with caution. A list of important flags
includes:

- `CARGO_TARGET_${TARGET}_LINKER`: specify a custom linker passed to rustc.
- `CARGO_TARGET_${TARGET}_RUNNER`: specify the wrapper to run executables.
- `CARGO_TARGET_${TARGET}_RUSTFLAGS`: add additional flags passed to rustc.

Any of the following [flags][cargo-flags] can be provided, and are converted to
uppercase. For example, changing `foo-bar` would be provided as
`CARGO_TARGET_${TARGET}_FOO_BAR`.

For example, to run binaries on `i686-unknown-linux-gnu` with Qemu, first
create a custom image containing Qemu, and run with the following command:

```
CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_RUNNER=qemu-i386 cross run ...
```


[cargo-flags]: https://doc.rust-lang.org/cargo/reference/config.html#target
