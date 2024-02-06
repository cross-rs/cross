<!--toc:start-->
- [Installing Cross](#installing-cross)
    - [Installing Rust via Rustup](#installing-rust-via-rustup)
    - [Installing Cross](#installing-cross)
- [Installing A Container Engine](#installing-a-container-engine)
- [Cross-Compiling Your First Package](#cross-compiling-your-first-package)
<!--toc:end-->

New to cross? Cross-compilation? Container engines? Here's how to get up-and-running.

# Installing Cross

## Installing Rust via Rustup

`cross` requires a `rustup` installation of Rust. To do so, the recommended
instructions are documented [here](https://www.rust-lang.org/tools/install),
but might differ on some platforms. For UNIX-like systems, run the following
command in a terminal and follow the instructions to install Rust and add Rust
to the path:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Windows, download
[rustup-init.exe](https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe)
or following the [other installation
methods](https://forge.rust-lang.org/infra/other-installation-methods.html),
say, to install from a package manager.

On some platforms, such as NixOS, you might need to use a package manager since
the default `rustup` install will fail. On NixOS, you should run the following,
which will install `rustup` and the latest `stable` release of Rust.

```bash
nix-env -i rustup
rustup toolchain install stable
```

Note that you might need additional tools on some platforms to get `rustc` and
`cargo` working. On UNIX-like systems, this generally means an install of GCC
or Clang. For example, on NixOS you will likely need to install GCC via
`nix-env -i gcc` and then go into a GCC and Rust shell (`nix-shell -p gcc
rustup`). On Alpine, you'll need to run `apk add libgcc gcc musl-dev`. Exact
instructions will differ by OS and Linux distro, feel free to ask on the
[discussion](https://github.com/cross-rs/cross/discussions) or our [Matrix
room](https://matrix.to/#/#cross-rs:matrix.org) if you have any questions.


## Installing Cross

Once `cargo` is installed via `rustup`, and the necessary additional tools are
present, you can now install `cross` via `cargo`:

```bash
cargo install cross
# Optionally, if you have cargo-binstall, you can install via pre-built binary
cargo binstall cross
```

Once `cross` is installed, you need a container engine and you can start
cross-compiling.


# Installing A Container Engine

On Windows and macOS, we generally recommend you use Docker unless you know
what you're doing. [Docker
Desktop](https://www.docker.com/products/docker-desktop/) install instructions
can be found [here](https://www.docker.com/products/docker-desktop/). On Linux,
you can either install via [Docker
Engine](https://docs.docker.com/engine/install/ubuntu/), [Docker
Desktop](https://docs.docker.com/desktop/install/linux-install/) or
[Podman](https://podman.io/getting-started/installation). We generally
recommend Podman, since it runs rootless by default. If you choose to use
Docker, make sure you add users to the [docker
group](https://docs.docker.com/engine/install/linux-postinstall/#manage-docker-as-a-non-root-user)
so it can be run without `sudo` (note that this has security implications) or
use [rootless](https://docs.docker.com/engine/security/rootless/)<sup>†</sup>
Docker.

If you use Docker Desktop for Windows, ensure you're using the WSL2. Follow the
[WSL2 installation
instructions](https://docs.microsoft.com/en-us/windows/wsl/install) to enable
the [WSL2 backend in docker](https://docs.docker.com/desktop/windows/wsl/).

Once your container engine is installed, you can check that it is running via:

```bash
# or use podman, if installed
$ docker ps -a
```

<sup>†</sup>Using rootless docker also requires setting the environment
variable `CROSS_ROOTLESS_CONTAINER_ENGINE=1`.


# Cross-Compiling Your First Package

Once both `cross` and the container engine are installed, you can build your
first package: this is all that's required.

```bash
$ cargo init --bin hello
$ cd hello
$ cross run --target aarch64-unknown-linux-gnu
   Compiling hello v0.1.0 (/project)
    Finished dev [unoptimized + debuginfo] target(s) in 0.64s
     Running `/linux-runner aarch64 /target/aarch64-unknown-linux-gnu/debug/hello`
Hello, world!
```

This will automatically install the Rust target required and the Docker image
containing the toolchain to cross-compile your target.

If you get an error similar to `error: toolchain
'stable-x86_64-unknown-linux-gnu' does not support components`, try
reinstalling that toolchain with rustup.

```sh
$ rustup toolchain uninstall stable-x86_64-unknown-linux-gnu
$ rustup toolchain install stable-x86_64-unknown-linux-gnu --force-non-host
```
