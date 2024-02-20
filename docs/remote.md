<!--toc:start-->
- [Getting Started](#getting-started)
- [Data Volumes](#data-volumes)
- [Managing Data](#managing-data)
- [Private Dependencies](#private-dependencies)
- [Environment Variables](#environment-variables)
<!--toc:end-->


# Getting Started

To inform `cross` it is using a remote container engine, set the environment
variable `CROSS_REMOTE=1`. Rather than use bind mounts to mount local volumes
into the filesystem, it copies data from the local filesystem into data volumes
on the remote host, ensuring all mounted volumes are present on the remote
filesystem in the same location as they would be present on the host. A sample
use with docker is:

```bash
CROSS_REMOTE=1 DOCKER_HOST=tcp://docker:2375/ cross build --target arm-unknown-linux-gnueabihf
```

If using [docker
contexts](https://docs.docker.com/engine/context/working-with-contexts/), you
do not need to provide `DOCKER_HOST`. This also works with podman and
podman-remote. For podman remote, make sure the connection is
[added](https://docs.podman.io/en/latest/markdown/podman-system-connection-add.1.html)
to podman, and if using a connection that requires authentication, that it is
set to the default identity, and that you do not use an identity (to avoid
being prompted for a password on every command).

An example with podman is:

```bash
podman system connection add cross tcp://localhost:8080 --default=true
CROSS_REMOTE=1 CROSS_CONTAINER_ENGINE=podman cross build --target arm-unknown-linux-gnueabihf
```

Any command using `podman` can be replaced with `podman-remote`. Cross
automatically detects if `podman` or `podman-remote` is being used, and adds in
the `--remote` flag if needed.


# Data Volumes

Since we cannot use bind mounts directly, we create a data volume mounted at
`/cross` in the container, and copy over all relevant data to that volume. To
ensure paths are identical with those on the host, all data is then symlinked
to the expected paths: for example, `/cross/home/user/project` becomes
`/home/user/project`. The files will have the expected metadata as on the host.

By default, `cross` does not copy the `cargo` registry, not the target
directory. These can be enabled via the `CROSS_REMOTE_COPY_REGISTRY` and
`CROSS_REMOTE_COPY_CACHE` environment variables. In order to minimize the
number of calls to docker, which has large overhead, when copying only subsets
of a directory, we copy all files to a temporary directory for faster
performance.

Since copying the entire toolchain remotely can take a long time, `cross` also
supports persistent data volumes containing all data for the current toolchain.
These can be created via:

```bash
cross-util volumes crate
```

`cross` will detect if a persistent data volume is present, and prefer it over
a single-use volume. The persistent data volume will also contain the project
files, and will reflect any changes to the local project by copying/removing
changed files on every build.


# Managing Data

Due to the addition of temporary files/directories, persistent data volumes,
and containers that may not be cleaned up, we've added utilities to clean up
data cross introduces:

```bash
# VOLUMES
# list all all persistent data volumes
$ cross-util volumes list
cross-stable-x86_64-unknown-linux-gnu-16b8c-fe5b13d68
# create a persistent data volume for the current toolchain
$ cross-util volumes create
# remove the persistent data volume for the current toolchain
$ cross-util volumes remove
# remove all persistent data volumes
$ cross-util volumes remove-all
# prune all data volumes not currently used with a container
# note that this affects more than just cross, and can
# be highly destructive
$ cross-util volumes prune

# CONTAINERS
# list all all hanging containers
$ cross-util containers list
# stop and remove all hanging containers
$ cross-util containers remove-all
```


# Private Dependencies

Note that for private dependencies, you will need to copy over the cargo
registry, since it uses the host toolchain, and might only support single-use
volumes (this has not been extensively tested):

```bash
CROSS_REMOTE_COPY_REGISTRY=1 CROSS_REMOTE=1 cross build --target arm-unknown-linux-gnueabihf
```

This is because the private dependencies are downloaded locally (to the host
registry), and therefore must be updated remotely, which will not have access
to SSH keys or other information inside the container.


# Environment Variables

Remote build behavior can be further customized by environment variables
provided to the build command.

- `CROSS_REMOTE`: Inform cross it is using a remote container engine, and use
  data volumes rather than local bind mounts. 
- `CROSS_REMOTE_COPY_REGISTRY`: Copy the `cargo` registry and git directories.
  Is needed to support  private SSH dependencies.
- `CROSS_REMOTE_COPY_CACHE`: Copy all directories, even those containing
  `CACHETAG.DIR` (a cache directory [tag](https://bford.info/cachedir/)).
- `CROSS_REMOTE_SKIP_BUILD_ARTIFACTS`: Do not copy any generated build
  artifacts back to the host after finishing the build. If using persistent
  data volumes, the artifacts will remain in the volume.

For additional environment variables, refer to the [environment variables
documentation][docs-env-vars].

[docs-env-vars]: ./environment_variables.md
