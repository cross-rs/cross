#![doc = "*** AUTO-GENERATED, do not touch. Run `cargo xtask codegen` to update ***"]
use super::{ImagePlatform, ProvidedImage};

#[rustfmt::skip]
pub static PROVIDED_IMAGES: &[ProvidedImage] = &[
        ProvidedImage {
            name: "x86_64-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-unknown-linux-musl",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: Some("centos")
        },
        ProvidedImage {
            name: "aarch64-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "arm-unknown-linux-gnueabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "arm-unknown-linux-gnueabihf",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "armv7-unknown-linux-gnueabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "armv7-unknown-linux-gnueabihf",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "thumbv7neon-unknown-linux-gnueabihf",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "i586-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "i686-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "mips-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "mipsel-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "mips64-unknown-linux-gnuabi64",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "mips64el-unknown-linux-gnuabi64",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "mips64-unknown-linux-muslabi64",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "mips64el-unknown-linux-muslabi64",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "powerpc-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "powerpc64-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "powerpc64le-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "riscv64gc-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "s390x-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "sparc64-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "aarch64-unknown-linux-musl",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "arm-unknown-linux-musleabihf",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "arm-unknown-linux-musleabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "armv5te-unknown-linux-gnueabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "armv5te-unknown-linux-musleabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "armv7-unknown-linux-musleabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "armv7-unknown-linux-musleabihf",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "i586-unknown-linux-musl",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "i686-unknown-linux-musl",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "mips-unknown-linux-musl",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "mipsel-unknown-linux-musl",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "aarch64-linux-android",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "arm-linux-androideabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "armv7-linux-androideabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "thumbv7neon-linux-androideabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "i686-linux-android",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-linux-android",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-pc-windows-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "i686-pc-windows-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "wasm32-unknown-emscripten",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-unknown-dragonfly",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "i686-unknown-freebsd",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-unknown-freebsd",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-unknown-netbsd",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "sparcv9-sun-solaris",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-sun-solaris",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "x86_64-unknown-illumos",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "thumbv6m-none-eabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "thumbv7em-none-eabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "thumbv7em-none-eabihf",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "thumbv7m-none-eabi",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "zig",
            platforms: &[ImagePlatform::DEFAULT],
            sub: None
        },
        ProvidedImage {
            name: "aarch64-unknown-linux-gnu",
            platforms: &[ImagePlatform::DEFAULT],
            sub: Some("centos")
        },
];
