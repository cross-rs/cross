use clap::Args;
use eyre::Context;
use std::{collections::BTreeMap, fmt::Write};

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
    let matrix = get_matrix()
        .iter()
        .filter(|i| i.to_image_target().is_toolchain_image())
        .collect::<Vec<_>>();
    let mut image_targets: BTreeMap<(String, Option<String>), Vec<String>> = BTreeMap::new();

    for entry in matrix {
        image_targets
            .entry((entry.target.clone(), entry.sub.clone()))
            .and_modify(|e| {
                e.extend(
                    entry
                        .platforms
                        .clone()
                        .unwrap_or_else(|| vec!["DEFAULT".to_string()]),
                )
            })
            .or_insert_with(|| {
                entry
                    .platforms
                    .clone()
                    .unwrap_or_else(|| vec!["DEFAULT".to_string()])
            });
    }

    for ((target, sub), platforms) in image_targets {
        write!(
            &mut images,
            r#"
        ProvidedImage {{
            name: "{}",
            platforms: &[{}],
            sub: {}
        }},"#,
            target,
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
                .join(", "),
            if let Some(sub) = sub {
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
