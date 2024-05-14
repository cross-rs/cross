<!--toc:start-->
- [`build`](#build)
- [`build.env`](#buildenv)
- [`build.dockerfile`](#builddockerfile)
- [`build.zig`](#buildzig)
- [`target.TARGET`](#targettarget)
- [`target.TARGET.pre-build`](#targettargetpre-build)
- [`target.TARGET.image`](#targettargetimage)
- [`target.TARGET.env`](#targettargetenv)
- [`target.TARGET.dockerfile`](#targettargetdockerfile)
- [`target.TARGET.zig`](#targettargetzig)
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


# `build`

The `build` key allows you to set global variables, e.g.:

> *NOTE*: `$CROSS_DEB_ARCH` is automatically provided by cross,
> [see here][custom_images_automatic_arch].

```toml
[build]
build-std = false                              # do not build the std library. has precedence over xargo
xargo = true                                   # enable the use of xargo by default
zig = false                                    # do not use zig cc for the builds
default-target = "x86_64-unknown-linux-gnu"    # use this target if none is explicitly provided
pre-build = [                                  # additional commands to run prior to building the package
    "dpkg --add-architecture $CROSS_DEB_ARCH", 
    "apt-get update && apt-get --assume-yes install libssl-dev:$CROSS_DEB_ARCH"
]                 
```


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


# `build.zig`

The `build.zig` key lets you use `zig cc` as a cross-compiler, enabling
cross-compilation to numerous architectures and glibc versions using a single
Docker image. Note that `zig cc` doesn't support all targets: only a subset of
our Linux GNU targets, so it might be better to set these values in
`target.TARGET.zig` instead. The value can be provided as either a table, a bool,
or a string. If `build.zig` is set to a string, it's equivalent to setting
`build.zig.version` to that value and `build.zig.enable` to true:

```toml
[build]
zig = "2.17"
```

If `build.zig` is set to a bool, it's equivalent to setting `build.zig.enable`
to that value:

```toml
[build]
zig = true
```

Or using a table:

```toml
[build.zig]
enable = true                 # enable or disable the use of zig cc
version = "2.17"              # the glibc version to use
image = "myimage"             # a custom image containing zig to use
```


# `target.TARGET`

The `target` key allows you to specify parameters for specific compilation
targets:

```toml
[target.aarch64-unknown-linux-gnu]
build-std = ["core", "alloc"]   # always build the `core` and `alloc` crates from the std library. has precedence over xargo
xargo = false                   # disable the use of xargo
image = "test-image"            # use a different image for the target
runner = "qemu-user"            # wrapper to run the binary (must be `qemu-system`, `qemu-user`, or `native`).
```


# `target.TARGET.pre-build`

The `pre-build` field can reference a file to copy and run. This file is
relative to the container context, which would be the workspace root, or the
current directory if `--manifest-path` is used. For more involved scripts,
consider using `target.TARGET.dockerfile` instead to directly control the
execution.

This script will be invoked as `RUN ./pre-build-script $CROSS_TARGET` where
`$CROSS_TARGET` is the target triple.

```toml
[target.aarch64-unknown-linux-gnu]
pre-build = "./scripts/my-script.sh"
```

```bash
$ cat ./scripts/my-script.sh
#!/usr/bin/env bash

apt-get install libssl-dev -y
```

`pre-build` can also be a list of commands to directly run inside the image:

> *NOTE*: `$CROSS_DEB_ARCH` is automatically provided by cross, [see
> here][custom_images_automatic_arch].

```toml
[target.aarch64-unknown-linux-gnu]
pre-build = [
    "dpkg --add-architecture $CROSS_DEB_ARCH",
    "apt-get update",
    "apt-get install --assume-yes libfoo:$CROSS_DEB_ARCH"
]
```


# `target.TARGET.image`

```toml
[target.aarch64-unknown-linux-gnu]
image = "my/image:latest"
```

In the example above, `cross` will use a image named `my/image:latest` instead of
the default one. Normal Docker behavior applies, so:

- Docker will first look for a local image named `my/image:latest`
- If it doesn't find a local image, then it will look in Docker Hub.
- If only `image:latest` is specified, then Docker won't look in Docker Hub.
- If the tag is omitted, then Docker will use the `latest` tag.

If you specify a tag but no image name, `cross` will use the default image with
the tag you provided:

```toml
[target.aarch64-unknown-linux-gnu]
# Translates to `ghcr.io/cross-rs/aarch64-unknown-linux-gnu:edge`
image = ":edge"

[target.x86_64-unknown-linux-musl]
# Translates to `ghcr.io/cross-rs/x86_64-unknown-linux-musl@sha256:77db671d8356a64ae72a3e1415e63f547f26d374fbe3c4762c1cd36c7eac7b99`
image = "@sha256:77db671d8356a64ae72a3e1415e63f547f26d374fbe3c4762c1cd36c7eac7b99"
```

You can also specify a subtarget with no tag nor image name:

```toml
[target.x86_64-unknown-linux-gnu]
# Translates to `ghcr.io/cross-rs/x86_64-unknown-linux-gnu:0.3.0-centos`
image = "-centos"
```

The `image` key can also take the toolchains/platforms supported by the image:

```toml
[target.aarch64-unknown-linux-gnu]
image.name = "alpine:edge"
image.toolchain = ["x86_64-unknown-linux-musl", "linux/arm64=aarch64-unknown-linux-musl"] # Defaults to `x86_64-unknown-linux-gnu`
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


# `target.TARGET.dockerfile`

The `dockerfile` key lets you provide a custom Docker image for the
given target. The value can be provided as either a table or a string. If
`target.TARGET.dockerfile` is set to a string, it's equivalent to setting
`target.(...).dockerfile.file` to that value. For example, using only a string:

```toml
[target.aarch64-unknown-linux-gnu]
dockerfile = "./Dockerfile"
```

Or using a table:

```toml
[target.aarch64-unknown-linux-gnu.dockerfile]
file = "./Dockerfile"         # the dockerfile to use relative to the `Cargo.toml`
context = "."                 # the context folder to build the script in. defaults to `.`
build-args = { ARG1 = "foo" } # https://docs.docker.com/engine/reference/builder/#arg
```


# `target.TARGET.zig`

The `target.TARGET.zig` key lets you use `zig cc` as a cross-compiler, enabling
cross-compilation to numerous architectures and glibc versions using a single
Docker image. The value can be provided as either a table, a bool, or a string.
If `target.TARGET.zig` is set to a string, it's equivalent to setting
`target.TARGET.zig.version` to that value and `target.TARGET.zig.enable` to
true:

```toml
[target.aarch64-unknown-linux-gnu]
zig = "2.17"
```

If `target.TARGET.zig` is set to a bool, it's equivalent to setting
`target.TARGET.zig.enable` to that value:

```toml
[target.aarch64-unknown-linux-gnu]
zig = true
```

Or using a table:

```toml
[target.aarch64-unknown-linux-gnu.zig]
enable = true                 # enable or disable the use of zig cc
version = "2.17"              # the glibc version to use
image = "myimage"             # a custom image containing zig to use
```



[example-cross-toml]: https://github.com/cross-rs/wiki_assets/blob/main/Configuration/Cross.toml
[example-cargo-toml]: https://github.com/cross-rs/wiki_assets/blob/main/Configuration/Cargo.toml
[custom_images_automatic_arch]: ./custom_images.md#automatic-target-architecture-on-debian
