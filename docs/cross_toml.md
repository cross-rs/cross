The `cross` configuration in the `Cross.toml` file, can contain the elements described below.

If the configuration is given in the `Cargo.toml`, these table headers must be of the form `[package.metadata.cross.<KEY>]`.

# `build`

The `build` key allows you to set global variables, e.g.:

```toml
[build]
xargo = true
build-std = true
default-target = "x86_64-unknown-linux-gnu"
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
image = "test-image"
pre-build = ["apt-get update"]
runner = "custom-runner"
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
