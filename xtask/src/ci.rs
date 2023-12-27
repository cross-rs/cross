mod target_matrix;

use crate::util::gha_output;
use clap::Subcommand;
use cross::shell::Verbosity;
use cross::{cargo_command, CargoMetadata, CommandExt};

#[derive(Subcommand, Debug)]
pub enum CiJob {
    /// Return needed metadata for building images
    PrepareMeta {
        // tag, branch
        #[clap(long, env = "GITHUB_REF_TYPE")]
        ref_type: String,
        // main, v0.1.0
        #[clap(long, env = "GITHUB_REF_NAME")]
        ref_name: String,
        target: crate::ImageTarget,
    },
    /// Check workspace metadata.
    Check {
        /// tag, branch
        #[clap(long, env = "GITHUB_REF_TYPE")]
        ref_type: String,
        /// main, v0.1.0
        #[clap(long, env = "GITHUB_REF_NAME")]
        ref_name: String,
    },
    TargetMatrix(target_matrix::TargetMatrix),
}

pub fn ci(args: CiJob, metadata: CargoMetadata) -> cross::Result<()> {
    let cross_meta = metadata
        .get_package("cross")
        .expect("cross expected in workspace");

    match args {
        CiJob::PrepareMeta {
            ref_type,
            ref_name,
            target,
        } => {
            // Set labels
            let mut labels = vec![];

            let image_title = match target.name.as_ref() {
                "cross" => target.name.to_string(),
                // TODO: Mention platform?
                _ => format!("cross (for {})", target.name),
            };
            labels.push(format!("org.opencontainers.image.title={image_title}"));
            labels.push(format!(
                "org.opencontainers.image.licenses={}",
                cross_meta.license.as_deref().unwrap_or_default()
            ));
            labels.push(format!(
                "org.opencontainers.image.created={}",
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            ));

            gha_output("labels", &serde_json::to_string(&labels.join("\n"))?)?;

            let version = cross_meta.version.clone();

            // Set image name
            gha_output(
                "image",
                &crate::build_docker_image::determine_image_name(
                    &target,
                    cross::docker::CROSS_IMAGE,
                    &ref_type,
                    &ref_name,
                    false,
                    &version,
                )?[0],
            )?;

            if target.has_ci_image() {
                gha_output("has-image", "true")?
            }
            if target.is_standard_target_image() {
                gha_output("test-variant", "default")?
            } else {
                gha_output("test-variant", &target.name)?
            }
        }
        CiJob::Check { ref_type, ref_name } => {
            let version = semver::Version::parse(&cross_meta.version)?;
            if ref_type == "tag" {
                if ref_name.starts_with('v') && ref_name != format!("v{version}") {
                    eyre::bail!("a version tag was published, but the tag does not match the current version in Cargo.toml");
                }
                let search = cargo_command()
                    .args(["search", "--limit", "1"])
                    .arg("cross")
                    .run_and_get_stdout(&mut Verbosity::Verbose(2).into())?;
                let (cross, rest) = search
                    .split_once(" = ")
                    .ok_or_else(|| eyre::eyre!("cargo search failed"))?;
                assert_eq!(cross, "cross");
                // Note: this version includes pre-releases.
                let latest_version = semver::Version::parse(
                    rest.split('"')
                        .nth(1)
                        .ok_or_else(|| eyre::eyre!("cargo search returned unexpected data"))?,
                )?;
                if version >= latest_version && version.pre.is_empty() {
                    gha_output("is-latest", "true")?
                }
            }
        }
        CiJob::TargetMatrix(target_matrix) => {
            target_matrix.run()?;
        }
    }
    Ok(())
}
