<!--toc:start-->
- [`build.env`](#buildenv)
- [`build.dockerfile`](#builddockerfile)
- [`target.TARGET.env`](#targettargetenv)
<!--toc:end-->

> **Note**: Additional configuration is available through
> [environment variables](./environment_variables.md)

You can place a `Cross.toml` file in the root of your Cargo project or use a
`CROSS_CONFIG` environment variable to tweak cross's behavior. You can also use
`package.metadata.cross.KEY` in `Cargo.toml`, and the priority of settings is
environment variables override `Cross.toml` options, which override
`Cargo.toml` options. Annotated examples of both
[`Cross.toml`][example-cross-toml] and [`Cargo.toml`][example-cargo-toml] are
provided.

For example, the `[build]` table in `Cross.toml` is identical to setting
`[package.metadata.cross.build]` in `Cargo.toml`.

The `cross` configuration in the `Cross.toml` file can contain the following
elements:


# `build.env`

With the `build.env` key you can globally set volumes that should be mounted in
the Docker container or environment variables that should be passed through.
For example:

```toml
[build.env]
volumes = ["VOL1_ARG", "VOL2_ARG=/path/to/volume"]
passthrough = ["VAR1_ARG", "VAR2_ARG=VALUE"]
```

Note how in the environment variable passthrough, we can provide a definition
for the variable as well. `VAR1_ARG` will be the value of the environment
variable on the host, while `VAR2_ARG` will be `VALUE`. Likewise, the path to
the volume for `VOL1_ARG` will be the value of the environment variable on the
host, while `VOL2_ARG` will be `/path/to/volume`.


# `build.dockerfile`

> If the image you want to use is already available from a container registry,
> check out the `target.TARGET.image` option below.

The `build.dockerfile` key lets you provide a custom Docker image for all
targets, except those specified `target.TARGET.dockerfile`. The value can be
provided as either a table or a string. If `build.dockerfile` is set to a
string, it's equivalent to setting `build.dockerfile.file` to that value. For
example, using only a string:

```toml
[build]
dockerfile = "./Dockerfile"
```

Or using a table:

```toml
[build.dockerfile]
file = "./Dockerfile"         # the dockerfile to use relative to the `Cargo.toml`
context = "."                 # the context folder to build the script in. defaults to `.`
build-args = { ARG1 = "foo" } # https://docs.docker.com/engine/reference/builder/#arg
```

`cross` will build and use the image that was built instead of the default
image. It's recommended to base your custom image on the default Docker image
that `cross` uses: `ghcr.io/cross-rs/{{TARGET}}:{{VERSION}}` (where
`{{VERSION}}` is `cross`'s version). This way you won't have to figure out how
to install a cross-C toolchain in your custom image.

> *NOTE*: `$CROSS_DEB_ARCH` is automatically provided by cross, [see
> here][custom_images_automatic_arch].

``` Dockerfile
FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest

RUN dpkg --add-architecture $CROSS_DEB_ARCH && \
    apt-get update && \
    apt-get install --assume-yes libfoo:$CROSS_DEB_ARCH
```

`cross` will provide the argument `CROSS_BASE_IMAGE` which points to the
default image `cross` would use for the target. Instead of the above, you can
also then do the following:

```Dockerfile
ARG CROSS_BASE_IMAGE
FROM $CROSS_BASE_IMAGE
RUN ...
```

# `target.TARGET.env`

The `env` key allows you to specify environment variables that should be used
for a specific compilation target. This is similar to `build.env`, but allows
you to be more specific per target:

```toml
[target.x86_64-unknown-linux-gnu.env]
volumes = ["VOL1_ARG", "VOL2_ARG=/path/to/volume"]
passthrough = ["VAR1_ARG", "VAR2_ARG=VALUE"]
```


