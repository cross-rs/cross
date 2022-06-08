#![deny(missing_debug_implementations, rust_2018_idioms)]

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Parser, Subcommand};
use cross::CommandExt;

// known image prefixes, with their registry
// the docker.io registry can also be implicit
const GHCR_IO: &str = "ghcr.io/cross-rs/";
const RUST_EMBEDDED: &str = "rustembedded/cross:";
const DOCKER_IO: &str = "docker.io/rustembedded/cross:";
const IMAGE_PREFIXES: &[&str] = &[GHCR_IO, DOCKER_IO, RUST_EMBEDDED];

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List cross images in local storage.
    ListImages {
        /// Provide verbose diagnostic output.
        #[clap(short, long)]
        verbose: bool,
        /// Container engine (such as docker or podman).
        #[clap(long)]
        engine: Option<String>,
    },
    /// Remove cross images in local storage.
    RemoveImages {
        /// If not provided, remove all images.
        targets: Vec<String>,
        /// Remove images matching provided targets.
        #[clap(short, long)]
        verbose: bool,
        /// Force removal of images.
        #[clap(short, long)]
        force: bool,
        /// Remove local (development) images.
        #[clap(short, long)]
        local: bool,
        /// Remove images. Default is a dry run.
        #[clap(short, long)]
        execute: bool,
        /// Container engine (such as docker or podman).
        #[clap(long)]
        engine: Option<String>,
    },
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
struct Image {
    repository: String,
    tag: String,
    // need to remove images by ID, not just tag
    id: String,
}

impl Image {
    fn name(&self) -> String {
        format!("{}:{}", self.repository, self.tag)
    }
}

fn get_container_engine(engine: Option<&str>) -> Result<PathBuf, which::Error> {
    if let Some(ce) = engine {
        which::which(ce)
    } else {
        cross::get_container_engine()
    }
}

fn parse_image(image: &str) -> Image {
    // this cannot panic: we've formatted our image list as `${repo}:${tag} ${id}`
    let (repository, rest) = image.split_once(':').unwrap();
    let (tag, id) = rest.split_once(' ').unwrap();
    Image {
        repository: repository.to_string(),
        tag: tag.to_string(),
        id: id.to_string(),
    }
}

fn is_cross_image(repository: &str) -> bool {
    IMAGE_PREFIXES.iter().any(|i| repository.starts_with(i))
}

fn is_local_image(tag: &str) -> bool {
    tag.starts_with("local")
}

fn get_cross_images(engine: &Path, verbose: bool, local: bool) -> cross::Result<Vec<Image>> {
    let stdout = Command::new(engine)
        .arg("images")
        .arg("--format")
        .arg("{{.Repository}}:{{.Tag}} {{.ID}}")
        .run_and_get_stdout(verbose)?;

    let mut images: Vec<Image> = stdout
        .lines()
        .map(parse_image)
        .filter(|image| is_cross_image(&image.repository))
        .filter(|image| local || !is_local_image(&image.tag))
        .collect();
    images.sort();

    Ok(images)
}

// the old rustembedded targets had the following format:
//  repository = (${registry}/)?rustembedded/cross
//  tag = ${target}(-${version})?
// our target triple must match `[A-Za-z0-9_-]`
fn rustembedded_target(tag: &str) -> String {
    let is_target_char = |c: char| c == '_' || c.is_ascii_alphanumeric();
    let mut components = vec![];
    for component in tag.split('-') {
        if !component.is_empty() && component.chars().all(is_target_char) {
            components.push(component)
        }
    }

    components.join("-")
}

fn get_image_target(image: &Image) -> cross::Result<String> {
    if let Some(stripped) = image.repository.strip_prefix(GHCR_IO) {
        Ok(stripped.to_string())
    } else if let Some(tag) = image.tag.strip_prefix(RUST_EMBEDDED) {
        Ok(rustembedded_target(tag))
    } else if let Some(tag) = image.tag.strip_prefix(DOCKER_IO) {
        Ok(rustembedded_target(tag))
    } else {
        eyre::bail!("cannot get target for image {}", image.name())
    }
}

fn list_images(engine: &Path, verbose: bool) -> cross::Result<()> {
    get_cross_images(engine, verbose, true)?
        .iter()
        .for_each(|line| println!("{}", line.name()));

    Ok(())
}

fn remove_images(
    engine: &Path,
    images: &[&str],
    verbose: bool,
    force: bool,
    execute: bool,
) -> cross::Result<()> {
    let mut command = Command::new(engine);
    command.arg("rmi");
    if force {
        command.arg("--force");
    }
    command.args(images);
    if execute {
        command.run(verbose)
    } else {
        println!("{:?}", command);
        Ok(())
    }
}

fn remove_all_images(
    engine: &Path,
    verbose: bool,
    force: bool,
    local: bool,
    execute: bool,
) -> cross::Result<()> {
    let images = get_cross_images(engine, verbose, local)?;
    let ids: Vec<&str> = images.iter().map(|i| i.id.as_ref()).collect();
    remove_images(engine, &ids, verbose, force, execute)
}

fn remove_target_images(
    engine: &Path,
    targets: &[String],
    verbose: bool,
    force: bool,
    local: bool,
    execute: bool,
) -> cross::Result<()> {
    let images = get_cross_images(engine, verbose, local)?;
    let mut ids = vec![];
    for image in images.iter() {
        let target = get_image_target(image)?;
        if targets.contains(&target) {
            ids.push(image.id.as_ref());
        }
    }
    remove_images(engine, &ids, verbose, force, execute)
}

pub fn main() -> cross::Result<()> {
    cross::install_panic_hook()?;
    let cli = Cli::parse();
    match &cli.command {
        Commands::ListImages { verbose, engine } => {
            let engine = get_container_engine(engine.as_deref())?;
            list_images(&engine, *verbose)?;
        }
        Commands::RemoveImages {
            targets,
            verbose,
            force,
            local,
            execute,
            engine,
        } => {
            let engine = get_container_engine(engine.as_deref())?;
            if targets.is_empty() {
                remove_all_images(&engine, *verbose, *force, *local, *execute)?;
            } else {
                remove_target_images(&engine, targets, *verbose, *force, *local, *execute)?;
            }
        }
    }

    Ok(())
}
