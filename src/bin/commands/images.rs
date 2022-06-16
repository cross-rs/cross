use std::collections::BTreeSet;

use clap::{Args, Subcommand};
use cross::{
    docker::{self, CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX},
    CommandExt, TargetList,
};

// known image prefixes, with their registry
// the docker.io registry can also be implicit
const GHCR_IO: &str = docker::CROSS_IMAGE;
const RUST_EMBEDDED: &str = "rustembedded/cross";
const DOCKER_IO: &str = "docker.io/rustembedded/cross";
const IMAGE_PREFIXES: &[&str] = &[GHCR_IO, DOCKER_IO, RUST_EMBEDDED];

#[derive(Args, Debug)]
pub struct ListImages {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// Only list images for a specific target
    pub target: String,
}

impl ListImages {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        list_images(self, &engine)
    }
}

#[derive(Args, Debug)]
pub struct RemoveImages {
    /// If not provided, remove all images.
    pub targets: Vec<String>,
    /// Remove images matching provided targets.
    #[clap(short, long)]
    pub verbose: bool,
    /// Force removal of images.
    #[clap(short, long)]
    pub force: bool,
    /// Remove local (development) images.
    #[clap(short, long)]
    pub local: bool,
    /// Remove images. Default is a dry run.
    #[clap(short, long)]
    pub execute: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl RemoveImages {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        if self.targets.is_empty() {
            remove_all_images(self, &engine)
        } else {
            remove_target_images(self, &engine)
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Images {
    /// List cross images in local storage.
    List(ListImages),
    /// Remove cross images in local storage.
    Remove(RemoveImages),
}

impl Images {
    pub fn run(self, engine: docker::Engine) -> cross::Result<()> {
        match self {
            Images::List(args) => args.run(engine),
            Images::Remove(args) => args.run(engine),
        }
    }

    pub fn engine(&self) -> Option<&str> {
        match self {
            Images::List(l) => l.engine.as_deref(),
            Images::Remove(l) => l.engine.as_deref(),
        }
    }

    pub fn verbose(&self) -> bool {
        match self {
            Images::List(l) => l.verbose,
            Images::Remove(l) => l.verbose,
        }
    }
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
struct Image {
    repository: String,
    tag: String,
    // need to remove images by ID, not just tag
    id: String,
}

impl std::fmt::Display for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.repository == "<none>" {
            f.write_str(&self.id)
        } else {
            f.write_str(&self.name())
        }
    }
}

impl Image {
    fn name(&self) -> String {
        format!("{}:{}", self.repository, self.tag)
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

fn get_cross_images(
    engine: &docker::Engine,
    verbose: bool,
    local: bool,
) -> cross::Result<Vec<Image>> {
    let mut images: BTreeSet<_> = cross::docker::subcommand(engine, "images")
        .args(&["--format", "{{.Repository}}:{{.Tag}} {{.ID}}"])
        .args(&[
            "--filter",
            &format!("label={}.for-cross-target", cross::CROSS_LABEL_DOMAIN),
        ])
        .run_and_get_stdout(verbose)?
        .lines()
        .map(parse_image)
        .collect();

    let stdout = cross::docker::subcommand(engine, "images")
        .args(&["--format", "{{.Repository}}:{{.Tag}} {{.ID}}"])
        .run_and_get_stdout(verbose)?;
    let ids: Vec<_> = images.iter().map(|i| i.id.to_string()).collect();
    images.extend(
        stdout
            .lines()
            .map(parse_image)
            .filter(|i| !ids.iter().any(|id| id == &i.id))
            .filter(|image| is_cross_image(&image.repository))
            .filter(|image| local || !is_local_image(&image.tag)),
    );

    Ok(images.into_iter().collect())
}

// the old rustembedded targets had the following format:
//  repository = (${registry}/)?rustembedded/cross
//  tag = ${target}(-${version})?
// the last component must match `[A-Za-z0-9_-]` and
// we must have at least 3 components. the first component
// may contain other characters, such as `thumbv8m.main-none-eabi`.
fn rustembedded_target(tag: &str) -> String {
    let is_target_char = |c: char| c == '_' || c.is_ascii_alphanumeric();
    let mut components = vec![];
    for (index, component) in tag.split('-').enumerate() {
        if index <= 2 || (!component.is_empty() && component.chars().all(is_target_char)) {
            components.push(component)
        } else {
            break;
        }
    }

    components.join("-")
}

fn get_image_target(
    engine: &cross::docker::Engine,
    image: &Image,
    target_list: &TargetList,
) -> cross::Result<String> {
    if let Some(stripped) = image.repository.strip_prefix(&format!("{GHCR_IO}/")) {
        return Ok(stripped.to_string());
    } else if let Some(tag) = image.tag.strip_prefix(RUST_EMBEDDED) {
        return Ok(rustembedded_target(tag));
    } else if let Some(tag) = image.tag.strip_prefix(DOCKER_IO) {
        return Ok(rustembedded_target(tag));
    } else if image
        .repository
        .starts_with(CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX)
    {
        if let Some(target) = target_list
            .triples
            .iter()
            .find(|target| image.tag.starts_with(target.as_str()))
            .cloned()
        {
            return Ok(target);
        }
    }
    let mut command = cross::docker::subcommand(engine, "inspect");
    command.args(&[
        "--format",
        &format!(
            r#"{{{{index .Config.Labels "{}.for-cross-target"}}}}"#,
            cross::CROSS_LABEL_DOMAIN
        ),
    ]);
    command.arg(&image.id);

    // TODO: verbosity = 3?
    let target = command.run_and_get_stdout(true)?;
    if target.trim().is_empty() {
        eyre::bail!("cannot get target for image {}", image)
    }
    Ok(target.trim().to_string())
}

pub fn list_images(
    ListImages { verbose, .. }: ListImages,
    engine: &docker::Engine,
) -> cross::Result<()> {
    get_cross_images(engine, verbose, true)?
        .iter()
        .for_each(|image| println!("{}", image));

    Ok(())
}

fn remove_images(
    engine: &docker::Engine,
    images: &[Image],
    verbose: bool,
    force: bool,
    execute: bool,
) -> cross::Result<()> {
    let mut command = docker::subcommand(engine, "rmi");
    if force {
        command.arg("--force");
    }
    command.args(images.iter().map(|i| &i.id));
    if images.is_empty() {
        Ok(())
    } else if execute {
        command.run(verbose, false).map_err(Into::into)
    } else {
        eprintln!("Note: this is a dry run. to remove the images, pass the `--execute` flag.");
        command.print_verbose(true);
        Ok(())
    }
}

pub fn remove_all_images(
    RemoveImages {
        verbose,
        force,
        local,
        execute,
        ..
    }: RemoveImages,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let images = get_cross_images(engine, verbose, local)?;
    remove_images(engine, &images, verbose, force, execute)
}

pub fn remove_target_images(
    RemoveImages {
        targets,
        verbose,
        force,
        local,
        execute,
        ..
    }: RemoveImages,
    engine: &docker::Engine,
) -> cross::Result<()> {
    let cross_images = get_cross_images(engine, verbose, local)?;
    let target_list = cross::rustc::target_list(false)?;
    let mut images = vec![];
    for image in cross_images {
        let target = dbg!(get_image_target(engine, &image, &target_list)?);
        if targets.contains(&target) {
            images.push(image);
        }
    }
    remove_images(engine, &images, verbose, force, execute)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rustembedded_target() {
        let targets = [
            "x86_64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "thumbv8m.main-none-eabi",
        ];
        for target in targets {
            let versioned = format!("{target}-0.2.1");
            assert_eq!(rustembedded_target(target), target.to_string());
            assert_eq!(rustembedded_target(&versioned), target.to_string());
        }
    }
}
