use std::collections::BTreeMap;
use std::path::Path;

use crate::util::{format_repo, pull_image};
use clap::Args;
use color_eyre::SectionExt;
use cross::rustc::VersionMetaExt;
use cross::{CommandExt, OutputExt};
use eyre::Context;

// Store raw text data in the binary so we don't need a data directory
// when extracting all targets, or running our target info script.
const READELF_SCRIPT: &str = include_str!("readelf.sh");

#[derive(Args, Debug)]
pub struct Readelf {
    /// If not provided, get info for all targets.
    targets: Vec<crate::ImageTarget>,
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    verbose: bool,
    /// Image registry.
    #[clap(long, default_value_t = String::from("ghcr.io"))]
    registry: String,
    /// Image repository.
    #[clap(long, default_value_t = String::from("cross-rs"))]
    repository: String,
    /// Image tag.
    #[clap(long, default_value_t = String::from("main"))]
    tag: String,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

type ReadelfArchitecture = BTreeMap<String, String>;

fn parse_readelf_architecture(string: &str) -> cross::Result<ReadelfArchitecture> {
    let (_, info) = string
        .split_once("File Attributes")
        .ok_or(eyre::eyre!("unable to find target info."))?;
    info.lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            l.trim()
                .strip_prefix("Tag_")
                .ok_or(eyre::eyre!("unable to strip tag from {l}"))
        })
        .map(|r| {
            r.and_then(|l| {
                l.split_once(": ")
                    .ok_or(eyre::eyre!("unable to split key/value for {l}"))
                    .map(|(k, v)| (k.to_string(), v.replace('"', "")))
            })
        })
        .collect::<cross::Result<ReadelfArchitecture>>()
}

fn target_architecture(
    engine: &Path,
    target: &cross::Target,
    image: &str,
    tag: &str,
    verbose: bool,
) -> cross::Result<()> {
    if !tag.starts_with("local") {
        pull_image(engine, image, verbose)?;
    }

    let host_version_meta =
        rustc_version::version_meta().wrap_err("couldn't fetch the `rustc` version")?;
    let host = host_version_meta.host();
    let cargo = home::cargo_home()?;
    let sysroot = cross::rustc::sysroot(&host, target, verbose)?;

    let engine = cross::docker::Engine::new(verbose)?;
    let mut command = cross::docker::subcommand(&engine, "run");
    command.arg("--rm");
    command.args(&["-e", &format!("TARGET={}", target.triple())]);
    command.args(&["-v", &format!("{}:/cargo:Z", cargo.display())]);
    command.args(&["-v", &format!("{}:/rust:Z,ro", sysroot.display())]);
    command.arg(image);
    command.args(&["bash", "-c", READELF_SCRIPT]);

    let out = command.run_and_get_output(verbose)?;
    let stdout = out.stdout()?;
    if stdout.trim().is_empty() {
        let stderr = out.stderr()?;
        if !stderr.trim().is_empty() {
            if stderr.contains("target may not be installed") {
                eprintln!("warning: target {target} is not installed");
                return Ok(());
            } else if !stderr.contains("Finished dev") {
                eprintln!("{}", stderr.header("stderr"));
                return Ok(());
            }
        }
        eprintln!("no target information: target `{target}` is likely same architecture as host.");
        if verbose {
            eprintln!("{}", stderr.header("stderr"));
        }
    } else {
        let arch = parse_readelf_architecture(&stdout)?;
        println!("{}", serde_json::to_string_pretty(&arch)?);
    }

    Ok(())
}

pub fn readelf(
    Readelf {
        mut targets,
        verbose,
        registry,
        repository,
        tag,
        ..
    }: Readelf,
    engine: &Path,
) -> cross::Result<()> {
    let matrix = crate::util::get_matrix();

    if targets.is_empty() {
        targets = matrix
            .iter()
            .map(|t| t.to_image_target())
            .filter(|t| t.has_ci_image())
            .collect();
    }

    for target in targets {
        let image = target.image_name(&format_repo(&registry, &repository), &tag);
        let target = cross::Target::BuiltIn {
            triple: target.triplet.clone(),
        };
        target_architecture(engine, &target, &image, &tag, verbose)?;
    }

    Ok(())
}
