# Target Notes

This page details target specific information when using `cross`.

## ARMv5

### Background

Currently, Rust's built-in definition for the ARM9 family of processors with glibc support (referred to as `armel-unknown-linux-gnueabi` by GCC, and `armv5te-unknown-linux-gnueabi` by Rust/LLVM) defines `max-atomic-width` as `0`, as there is no hardware-level support for atomic operations. However when linked with `libgcc`, there are atomic primatives provided by the operating system. Additionally, this target uses the system allocator rather than jemalloc, as jemalloc does not currently build for this target. `unwind` is also not supported, so `panic` will abort.

This `cross` target uses those OS intrinsics to provided atomic operations, and uses the gcc name of `armel-unknown-linux-gnueabi` to avoid aliasing the builtin specification.

Previous discussion of this topic is available [in this reddit thread](https://www.reddit.com/r/rust/comments/64j39d/crosscompiling_for_arm926ejs/)

### Integration steps

Depending on which version of Rust you are using, the integration steps differ.

#### Rust 1.19.0 and before

Edit your `Cargo.toml`. Make sure the following sections are included:

```toml
[profile.release]
panic = "abort"

[profile.dev]
debug = true
panic = "abort"
```

Create a file in your project directory (next to `Cargo.toml`) called `Xargo.toml`. This file should have the following contents:

```toml
[target.armel-unknown-linux-gnueabi.dependencies.std]
default-features = false
features=["panic_abort", "force_alloc_system"]
```

#### Rust 1.20.0 and above

Edit your `Cargo.toml`. Make sure the following sections are included:

```toml
[profile.release]
panic = "abort"

[profile.dev]
debug = true
panic = "abort"
```

Create a file in your project directory (next to `Cargo.toml`) called `Xargo.toml`. This file should have the following contents:

```toml
[target.armel-unknown-linux-gnueabi.dependencies.std]
default-features = false
features=["panic_abort"]
```

In your `main.rs`, add this to the top of your file:

```rust
#![feature(global_allocator)]
#![feature(allocator_api)]

use std::heap::System;

#[global_allocator]
static mut SYSTEM_ALLOCATOR: System = System;
```