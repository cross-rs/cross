use clap::Args;
use cross::docker::ImagePlatform;
use eyre::Context;
use std::fmt::Write;

use crate::util::{get_cargo_workspace, get_matrix};

#[derive(Args, Debug)]
pub struct Codegen {}

pub fn codegen(Codegen { .. }: Codegen) -> cross::Result<()> {
    let path = get_cargo_workspace().join("src/docker/provided_images.rs");
    std::fs::write(path, docker_images()).wrap_err("when writing src/docker/provided_images.rs")?;
    Ok(())
}

pub fn docker_images() -> String {
    let mut images = String::from(
        r#"#![doc = "*** AUTO-GENERATED, do not touch. Run `cargo xtask codegen` to update ***"]
use super::{ImagePlatform, ProvidedImage};

#[rustfmt::skip]
pub static PROVIDED_IMAGES: &[ProvidedImage] = &["#,
    );

    for image_target in get_matrix()
        .iter()
        .filter(|i| i.builds_image() && i.to_image_target().is_toolchain_image() && !i.disabled)
    {
        write!(
            &mut images,
            r#"
        ProvidedImage {{
            name: "{name}",
            platforms: &[{platform}],
            sub: {sub}
        }},"#,
            name = image_target.target.clone(),
            platform = &image_target
                .platforms()
                .iter()
                .map(|p| {
                    let image_platform: ImagePlatform =
                        p.parse().expect("should be a valid platform");

                    image_platform
                        .to_codegen_string()
                        .expect("should be a valid platform")
                })
                .collect::<Vec<_>>()
                .join(", "),
            sub = if let Some(sub) = &image_target.sub {
                format!(r#"Some("{}")"#, sub)
            } else {
                "None".to_string()
            }
        )
        .expect("writing to string should not fail")
    }

    images.push_str("\n];\n");
    images
}

#[cfg(test)]
#[test]
pub fn ensure_correct_codegen() -> cross::Result<()> {
    let provided_images = crate::util::get_cargo_workspace().join("src/docker/provided_images.rs");
    let content = cross::file::read(provided_images)?;
    assert_eq!(content.replace("\r\n", "\n"), docker_images());
    Ok(())
}
