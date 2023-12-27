use std::collections::{BTreeMap, BTreeSet};

use clap::builder::PossibleValue;
use clap::{Args, Subcommand};
use cross::docker::{self, CROSS_CUSTOM_DOCKERFILE_IMAGE_PREFIX};
use cross::shell::MessageInfo;
use cross::{CommandExt, TargetList};

// known image prefixes, with their registry
// the docker.io registry can also be implicit
const GHCR_IO: &str = docker::CROSS_IMAGE;
const RUST_EMBEDDED: &str = "rustembedded/cross";
const DOCKER_IO: &str = "docker.io/rustembedded/cross";
const IMAGE_PREFIXES: &[&str] = &[GHCR_IO, DOCKER_IO, RUST_EMBEDDED];

#[derive(Args, Debug)]
pub struct ListImages {
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
    /// Output format
    #[clap(long, default_value = "human")]
    pub format: OutputFormat,
    /// Only list images for specific target(s). By default, list all targets.
    pub targets: Vec<String>,
}

impl ListImages {
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        list_images(self, &engine, msg_info)
    }
}

#[derive(Clone, Debug)]
pub enum OutputFormat {
    Human,
    Json,
}

impl clap::ValueEnum for OutputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Human, Self::Json]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            OutputFormat::Human => Some(PossibleValue::new("human")),
            OutputFormat::Json => Some(PossibleValue::new("json")),
        }
    }
}

#[derive(Args, Debug)]
pub struct RemoveImages {
    /// If not provided, remove all images.
    pub targets: Vec<String>,
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
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        if self.targets.is_empty() {
            remove_all_images(self, &engine, msg_info)
        } else {
            remove_target_images(self, &engine, msg_info)
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
    pub fn run(&self, engine: docker::Engine, msg_info: &mut MessageInfo) -> cross::Result<()> {
        match self {
            Images::List(args) => args.run(engine, msg_info),
            Images::Remove(args) => args.run(engine, msg_info),
        }
    }

    pub fn engine(&self) -> Option<&str> {
        match self {
            Images::List(l) => l.engine.as_deref(),
            Images::Remove(l) => l.engine.as_deref(),
        }
    }
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, serde::Serialize)]
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
    msg_info: &mut MessageInfo,
    local: bool,
) -> cross::Result<Vec<Image>> {
    let mut images: BTreeSet<_> = engine
        .subcommand("images")
        .args(["--format", "{{.Repository}}:{{.Tag}} {{.ID}}"])
        .args([
            "--filter",
            &format!("label={}.for-cross-target", cross::CROSS_LABEL_DOMAIN),
        ])
        .run_and_get_stdout(msg_info)?
        .lines()
        .map(parse_image)
        .collect();

    let stdout = engine
        .subcommand("images")
        .args(["--format", "{{.Repository}}:{{.Tag}} {{.ID}}"])
        .run_and_get_stdout(msg_info)?;
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
    msg_info: &mut MessageInfo,
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
    let mut command = engine.subcommand("inspect");
    command.args([
        "--format",
        &format!(
            r#"{{{{index .Config.Labels "{}.for-cross-target"}}}}"#,
            cross::CROSS_LABEL_DOMAIN
        ),
    ]);
    command.arg(&image.id);

    let target = command.run_and_get_stdout(msg_info)?;
    if target.trim().is_empty() {
        eyre::bail!("cannot get target for image {}", image)
    }
    Ok(target.trim().to_string())
}

pub fn list_images(
    ListImages {
        targets, format, ..
    }: &ListImages,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let cross_images = get_cross_images(engine, msg_info, true)?;
    let target_list = msg_info.as_quiet(cross::rustc::target_list)?;
    let mut map: BTreeMap<String, Vec<Image>> = BTreeMap::new();
    let mut max_target_len = 0;
    let mut max_image_len = 0;
    for image in cross_images {
        let target = get_image_target(engine, &image, &target_list, msg_info)?;
        if targets.is_empty() || targets.contains(&target) {
            if !map.contains_key(&target) {
                map.insert(target.clone(), vec![]);
            }
            max_target_len = target.len().max(max_target_len);
            max_image_len = image.name().len().max(max_image_len);
            map.get_mut(&target).expect("map must have key").push(image);
        }
    }
    let mut keys: Vec<&str> = map.keys().map(|k| k.as_ref()).collect();
    keys.sort_unstable();

    match format {
        OutputFormat::Json => {
            msg_info.info(format_args!("{}", serde_json::to_string(&map)?))?;
        }
        OutputFormat::Human => {
            let print_string =
                |col1: &str, col2: &str, fill: char, info: &mut MessageInfo| -> cross::Result<()> {
                    let mut row = String::new();
                    row.push('|');
                    row.push(fill);
                    row.push_str(col1);
                    let spaces = max_target_len.max(col1.len()) + 1 - col1.len();
                    for _ in 0..spaces {
                        row.push(fill);
                    }
                    row.push('|');
                    row.push(fill);
                    row.push_str(col2);
                    let spaces = max_image_len.max(col2.len()) + 1 - col2.len();
                    for _ in 0..spaces {
                        row.push(fill);
                    }
                    row.push('|');
                    info.print(row)
                };

            if targets.len() != 1 {
                print_string("Targets", "Images", ' ', msg_info)?;
                print_string("-------", "------", '-', msg_info)?;
            }

            let print_single = |_: &str,
                                image: &Image,
                                info: &mut MessageInfo|
             -> cross::Result<()> { info.print(image) };
            let print_table =
                |target: &str, image: &Image, info: &mut MessageInfo| -> cross::Result<()> {
                    let name = image.name();
                    print_string(target, &name, ' ', info)
                };

            for target in keys {
                for image in map.get(target).expect("map must have key").iter() {
                    if targets.len() == 1 {
                        print_single(target, image, msg_info)?;
                    } else {
                        print_table(target, image, msg_info)?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn remove_images(
    engine: &docker::Engine,
    images: &[Image],
    msg_info: &mut MessageInfo,
    force: bool,
    execute: bool,
) -> cross::Result<()> {
    let mut command = engine.subcommand("rmi");
    if force {
        command.arg("--force");
    }
    command.args(images.iter().map(|i| &i.id));
    if images.is_empty() {
        Ok(())
    } else if execute {
        command.run(msg_info, false)
    } else {
        msg_info.note("this is a dry run. to remove the images, pass the `--execute` flag.")?;
        command.print(msg_info)?;
        Ok(())
    }
}

pub fn remove_all_images(
    RemoveImages {
        force,
        local,
        execute,
        ..
    }: &RemoveImages,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let images = get_cross_images(engine, msg_info, *local)?;
    remove_images(engine, &images, msg_info, *force, *execute)
}

pub fn remove_target_images(
    RemoveImages {
        targets,
        force,
        local,
        execute,
        ..
    }: &RemoveImages,
    engine: &docker::Engine,
    msg_info: &mut MessageInfo,
) -> cross::Result<()> {
    let cross_images = get_cross_images(engine, msg_info, *local)?;
    let target_list = msg_info.as_quiet(cross::rustc::target_list)?;
    let mut images = vec![];
    for image in cross_images {
        let target = get_image_target(engine, &image, &target_list, msg_info)?;
        if targets.contains(&target) {
            images.push(image);
        }
    }
    remove_images(engine, &images, msg_info, *force, *execute)
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
