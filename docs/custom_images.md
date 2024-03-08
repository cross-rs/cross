<!--toc:start-->
- [Automatic Target Architecture on Debian](#automatic-target-architecture-on-debian)
- [Custom Images](#custom-images)
  - [Adding Dependencies to Existing Images](#adding-dependencies-to-existing-images)
  - [Custom Dockerfile](#custom-dockerfile)
  - [Custom Image](#custom-image)
<!--toc:end-->

# Automatic Target Architecture on Debian

Custom images generated from config `dockerfile` or `pre-build` keys will
export `CROSS_DEB_ARCH`, which allows you to install packages from
Ubuntu/Debian repositories without having to specify the exact architecture.
You can find an
[example of this here](#adding-dependencies-to-existing-images).


# Custom Images

`cross` provides default Docker images for the targets listed [in the
README](../README.md#supported-targets). However, it can't cover every single
use case out there.

## Adding Dependencies to Existing Images

If you simply need to install a dependency availaible in ubuntus package
manager, see [`target.TARGET.pre-build`][config-target-pre-build]:

```toml
[target.x86_64-unknown-linux-gnu]
pre-build = [
    "dpkg --add-architecture $CROSS_DEB_ARCH",
    "apt-get update && apt-get install --assume-yes libssl-dev:$CROSS_DEB_ARCH"
]
```

For FreeBSD targets, a few helper scripts are available for use in
[`target.TARGET.pre-build`][config-target-pre-build]:

```toml
[target.x86_64-unknown-freebsd]
pre-build = ["""
export FREEBSD_MIRROR=$(/freebsd-fetch-best-mirror.sh) &&
/freebsd-setup-packagesite.sh &&
/freebsd-install-package.sh xen-tools
"""]
```

## Custom Dockerfile

For other targets, or when the default image is not enough, you can use the
[`target.{{TARGET}}.dockerfile`][config_target_dockerfile] field
in `Cross.toml` to use a custom Docker image for a specific target:

> *NOTE*: Refer to the [`build.dockerfile`][config_build_dockerfile] section of
> the configuration for tips when writing your own `Dockerfile`.

``` toml
[target.aarch64-unknown-linux-gnu]
dockerfile = "Dockerfile"
```

`cross` will build and use the image that was built instead of the default
image.


## Custom Image

If there is a pre-built image for your specific target, you can use the
[`target.{{TARGET}}.image`][config_target_image] field in `Cross.toml` to use
that instead:

``` toml
[target.aarch64-unknown-linux-gnu]
image = "my/image:tag"
```

In thie case, `cross` will use a image named `my/image:tag` instead of the
default one. Normal Docker behavior applies, so:

- Docker will first look for a local image named `my/image:tag`
- If it doesn't find a local image, then it will look in Docker Hub.
- If only `image:tag` is specified, then Docker won't look in Docker Hub.
- If only `tag` is omitted, then Docker will use the `latest` tag.



[config-target-pre-build]: ./config_file.md#targettargetpre-build
[config_target_dockerfile]: ./config_file.md#targettargetdockerfile
[config_target_image]: ./config_file.md#targettargetimage
[config_build_dockerfile]: ./config_file.md#builddockerfile
