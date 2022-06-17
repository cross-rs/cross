use clap::Args;
use eyre::Context;
use std::fmt::Write;

use crate::util::{get_cargo_workspace, get_matrix};

#[derive(Args, Debug)]
pub struct Codegen {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    verbose: bool,
}

pub fn codegen(Codegen { .. }: Codegen) -> cross::Result<()> {
    let path = get_cargo_workspace().join("src/docker/provided_images.rs");
    std::fs::write(path, docker_images()).wrap_err("when writing src/docker/provided_images.rs")?;
    Ok(())
}

pub fn docker_images() -> String {
    let mut images = String::from(
        r##"#![doc = "*** AUTO-GENERATED, do not touch. Run `cargo xtask codegen` to update ***"]
use super::{ImagePlatform, ProvidedImage};

#[rustfmt::skip]
pub static PROVIDED_IMAGES: &[ProvidedImage] = &["##,
    );

    for image_target in get_matrix()
        .iter()
        .filter(|i| i.to_image_target().is_standard_target_image())
    {
        write!(
            &mut images,
            r#"
        ProvidedImage {{
            name: "{}",
            platforms: &[{}],
            sub: {}
        }},"#,
            image_target.target.clone(),
            if let Some(platforms) = &image_target.platforms {
                platforms
                    .iter()
                    .map(|p| {
                        format!(
                            "ImagePlatform::{}",
                            p.replace('-', "_").to_ascii_uppercase()
                        )
                    })
                    .collect::<Vec<_>>()
                    .as_slice()
                    .join(", ")
            } else {
                "ImagePlatform::DEFAULT".to_string()
            },
            if let Some(sub) = &image_target.sub {
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
