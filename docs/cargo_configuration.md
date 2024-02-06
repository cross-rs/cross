<!--toc:start-->
- [Use Xargo instead of Cargo](#use-xargo-instead-of-cargo)
<!--toc:end-->


# Use Xargo instead of Cargo

By default, `cross` uses `xargo` to build your Cargo project only for all
non-standard targets (i.e. something not reported by rustc/rustup). However,
you can use the `build.xargo` or `target.{{TARGET}}.xargo` field in
`Cross.toml` to force the use of `xargo`:

```toml
# all the targets will use `xargo`
[build]
xargo = true
```

Or,

```toml
# only this target will use `xargo`
[target.aarch64-unknown-linux-gnu]
xargo = true
```

`xargo = false` will work the opposite way (pick cargo always) and is useful
when building for custom targets that you know to work with cargo.


[cargo-flags]: https://doc.rust-lang.org/cargo/reference/config.html#target
