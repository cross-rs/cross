The `cross` configuration in the `Cross.toml` file, can contain the following elements:

# `build`
The `build` key allows you to set global variables, e.g.:

```toml
[build]
xargo = true
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
volumes = ["VOL1_ARG", "VOL2_ARG"]
passthrough = ["VAR1", "VAR2"]
xargo = false
image = "test-image"
runner = "custom-runner"
```
