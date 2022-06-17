use std::path::Path;

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum CiJob {
    PrepareMeta {
        // tag, branch
        #[clap(long, env = "GITHUB_REF_TYPE")]
        ref_type: String,
        // main, v0.1.0
        #[clap(long, env = "GITHUB_REF_NAME")]
        ref_name: String,
        target: crate::ImageTarget,
    },
}

pub fn ci(args: CiJob) -> cross::Result<()> {
    match args {
        CiJob::PrepareMeta {
            ref_type,
            ref_name,
            target,
        } => {
            let metadata = cross::cargo_metadata_with_args(
                Some(Path::new(env!("CARGO_MANIFEST_DIR"))),
                None,
                true,
            )?
            .ok_or_else(|| eyre::eyre!("could not find cross workspace and its current version"))?;
            let cross_meta = metadata
                .get_package("cross")
                .expect("cross expected in workspace");

            // Set labels
            let mut labels = vec![];

            labels.push(format!(
                "org.opencontainers.image.title=cross (for {})",
                target.triplet
            ));
            labels.push(format!(
                "org.opencontainers.image.licenses={}",
                cross_meta.license.as_deref().unwrap_or_default()
            ));

            gha_output("labels", &serde_json::to_string(&labels.join("\n"))?);

            let version = cross_meta.version.clone();

            // Set image name
            gha_output(
                "image",
                &crate::build_docker_image::determine_image_name(
                    &target,
                    cross::docker::CROSS_IMAGE,
                    &ref_type,
                    &ref_name,
                    &version,
                )?[0],
            );

            if target.has_ci_image() {
                gha_output("has-image", "true")
            }
        }
    }
    Ok(())
}

#[track_caller]
fn gha_output(tag: &str, content: &str) {
    if content.contains('\n') {
        // https://github.com/actions/toolkit/issues/403
        panic!("output `{tag}` contains newlines, consider serializing with json and deserializing in gha with fromJSON()")
    }
    println!("::set-output name={tag}::{}", content)
}
