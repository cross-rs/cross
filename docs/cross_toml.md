The `cross` configuration in the `Cross.toml` file, can contain the following elements:

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
