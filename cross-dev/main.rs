#![deny(missing_debug_implementations, rust_2018_idioms)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};
use cross::CommandExt;
use serde::Deserialize;

// Store raw text data in the binary so we don't need a data directory
// when extracting all targets, or running our target info script.
const TARGET_INFO_SCRIPT: &str = include_str!("target_info.sh");
const WORKFLOW: &str = include_str!("../.github/workflows/ci.yml");

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Extract and print info for targets.
    TargetInfo {
        /// If not provided, get info for all targets.
        targets: Vec<String>,
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
        engine: Option<String>,
    },
}

#[derive(Debug, PartialEq, Deserialize)]
struct Workflow {
    jobs: Jobs,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Jobs {
    #[serde(rename = "generate-matrix")]
    generate_matrix: GenerateMatrix,
}

#[derive(Debug, PartialEq, Deserialize)]
struct GenerateMatrix {
    steps: Vec<Steps>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Steps {
    env: Env,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Env {
    matrix: String,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Matrix {
    target: String,
    #[serde(default)]
    run: i64,
}

impl Matrix {
    fn has_test(&self, target: &str) -> bool {
        // bare-metal targets don't have unittests right now
        self.run != 0 && !target.contains("-none-")
    }
}

fn get_container_engine(engine: Option<&str>) -> Result<PathBuf, which::Error> {
    if let Some(ce) = engine {
        which::which(ce)
    } else {
        cross::get_container_engine()
    }
}

fn target_has_image(target: &str) -> bool {
    let imageless = ["-msvc", "-darwin", "-apple-ios"];
    !imageless.iter().any(|t| target.ends_with(t))
}

fn format_image(registry: &str, repository: &str, target: &str, tag: &str) -> String {
    let mut output = format!("{target}:{tag}");
    if !repository.is_empty() {
        output = format!("{repository}/{output}");
    }
    if !registry.is_empty() {
        output = format!("{registry}/{output}");
    }

    output
}

fn pull_image(engine: &Path, image: &str, verbose: bool) -> cross::Result<()> {
    let mut command = Command::new(engine);
    command.arg("pull");
    command.arg(image);
    if !verbose {
        // capture output to avoid polluting table
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
    }
    command.run(verbose)
}

fn image_info(
    engine: &Path,
    target: &str,
    image: &str,
    tag: &str,
    verbose: bool,
    has_test: bool,
) -> cross::Result<()> {
    if !tag.starts_with("local") {
        pull_image(engine, image, verbose)?;
    }

    let mut command = Command::new(engine);
    command.arg("run");
    command.arg("-it");
    command.arg("--rm");
    command.args(&["-e", &format!("TARGET={target}")]);
    if has_test {
        command.args(&["-e", "HAS_TEST=1"]);
    }
    command.arg(image);
    command.args(&["bash", "-c", TARGET_INFO_SCRIPT]);

    if !verbose {
        // capture stderr to avoid polluting table
        command.stderr(Stdio::null());
    }
    command.run(verbose)
}

fn target_info(
    mut targets: Vec<String>,
    engine: &Path,
    verbose: bool,
    registry: &str,
    repository: &str,
    tag: &str,
) -> cross::Result<()> {
    let workflow: Workflow = serde_yaml::from_str(WORKFLOW)?;
    let matrix = &workflow.jobs.generate_matrix.steps[0].env.matrix;
    let matrix: Vec<Matrix> = serde_yaml::from_str(matrix)?;
    let test_map: BTreeMap<&str, bool> = matrix
        .iter()
        .map(|i| (i.target.as_ref(), i.has_test(&i.target)))
        .collect();

    if targets.is_empty() {
        targets = matrix
            .iter()
            .map(|t| t.target.clone())
            .filter(|t| target_has_image(t))
            .collect();
    }

    for target in targets {
        let target = target.as_ref();
        let image = format_image(registry, repository, target, tag);
        let has_test = test_map
            .get(&target)
            .cloned()
            .ok_or_else(|| eyre::eyre!("invalid target name {}", target))?;
        image_info(engine, target, &image, tag, verbose, has_test)?;
    }

    Ok(())
}

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let cli = Cli::parse();
    match &cli.command {
        Commands::TargetInfo {
            targets,
            verbose,
            registry,
            repository,
            tag,
            engine,
        } => {
            let engine = get_container_engine(engine.as_deref())?;
            target_info(
                targets.to_vec(),
                &engine,
                *verbose,
                registry,
                repository,
                tag,
            )?;
        }
    }

    Ok(())
}
