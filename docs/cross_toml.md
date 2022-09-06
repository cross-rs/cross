The `cross` configuration in the `Cross.toml` file, can contain the elements described below.

If the configuration is given in the `Cargo.toml`, these table headers must be of the form `[package.metadata.cross.<KEY>]`.

# `build`

The `build` key allows you to set global variables, e.g.:

```toml
[build]
xargo = true
build-std = true
default-target = "x86_64-unknown-linux-gnu"
pre-build = ["apt-get update"] # can also be the path to a file to run
```

# `build.env`

With the `build.env` key you can globally set volumes that should be mounted
in the Docker container or environment variables that should be passed through.
For example:

```toml
[build.env]
volumes = ["VOL1_ARG", "VOL2_ARG"]
passthrough = ["IMPORTANT_ENV_VARIABLES"]
```

# `target.TARGET`

The `target` key allows you to specify parameters for specific compilation targets.

```toml
[target.aarch64-unknown-linux-gnu]
xargo = false
build-std = false
zig = "2.17"
image = "test-image"
pre-build = ["apt-get update"] # can also be the path to a file to run
runner = "custom-runner"
```

# `target.TARGET.pre-build`

The `pre-build` field can also reference a file to copy and run. This file is relative to the container context, which would be the workspace root, or the current directory if `--manifest-path` is used. For more involved scripts, consider using `target.TARGET.dockerfile` instead to directly control the execution.

This script will be invoked as `RUN ./pre-build-script $CROSS_TARGET` where `$CROSS_TARGET` is the target triple.

```toml
[target.aarch64-unknown-linux-gnu]
pre-build = "./scripts/my-script.sh"
```

```sh
$ cat ./scripts/my-script.sh
#!/usr/bin/env bash

apt-get install libssl-dev -y
```

# `target.TARGET.image`

The `image` key can also take the toolchains/platforms supported by the image.

```toml
[target.aarch64-unknown-linux-gnu]
image.name = "alpine:edge"
image.toolchain = ["x86_64-unknown-linux-musl", "linux/arm64=aarch64-unknown-linux-musl"] # Defaults to `x86_64-unknown-linux-gnu`
```

# `target.TARGET.env`

The `target` key allows you to specify environment variables that should be used for a specific compilation target.
This is similar to `build.env`, but allows you to be more specific per target.

```toml
[target.x86_64-unknown-linux-gnu.env]
volumes = ["VOL1_ARG", "VOL2_ARG"]
passthrough = ["IMPORTANT_ENV_VARIABLES"]
```

# `target.TARGET.dockerfile`

```toml
[target.x86_64-unknown-linux-gnu.dockerfile]
file = "./Dockerfile" # The dockerfile to use relative to the `Cargo.toml`
context = "." # What folder to run the build script in
build-args = { ARG1 = "foo" } # https://docs.docker.com/engine/reference/builder/#arg
```

also supports

```toml
[target.x86_64-unknown-linux-gnu]
dockerfile = "./Dockerfile"
```

# `target.TARGET.zig`

```toml
[target.x86_64-unknown-linux-gnu.zig]
enable = true       # enable use of the zig image
version = "2.17"    # glibc version to use
image = "zig:local" # custom zig image to use
```

also supports

```toml
[target.x86_64-unknown-linux-gnu]
zig = true
```

or

```toml
[target.x86_64-unknown-linux-gnu]
zig = "2.17"
```
