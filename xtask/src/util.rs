use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use cross::shell::MessageInfo;
use cross::{docker, CommandExt, ToUtf8};
use once_cell::sync::OnceCell;
use serde::Deserialize;

const WORKFLOW: &str = include_str!("../../.github/workflows/ci.yml");

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Workflow {
    jobs: Jobs,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Jobs {
    #[serde(rename = "generate-matrix")]
    generate_matrix: GenerateMatrix,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct GenerateMatrix {
    steps: Vec<Steps>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Steps {
    env: Env,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Env {
    matrix: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Matrix {
    pub target: String,
    pub sub: Option<String>,
    #[serde(default)]
    pub run: i64,
    pub os: String,
}

impl Matrix {
    pub fn has_test(&self, target: &str) -> bool {
        // bare-metal targets don't have unittests right now
        self.run != 0 && !target.contains("-none-")
    }

    pub fn to_image_target(&self) -> crate::ImageTarget {
        crate::ImageTarget {
            triplet: self.target.clone(),
            sub: self.sub.clone(),
        }
    }

    fn builds_image(&self) -> bool {
        self.os == "ubuntu-latest"
    }
}

static MATRIX: OnceCell<Vec<Matrix>> = OnceCell::new();

pub fn get_matrix() -> &'static Vec<Matrix> {
    MATRIX
        .get_or_try_init::<_, eyre::Report>(|| {
            let workflow: Workflow = serde_yaml::from_str(WORKFLOW)?;
            let matrix = &workflow.jobs.generate_matrix.steps[0].env.matrix;
            serde_yaml::from_str(matrix).map_err(Into::into)
        })
        .unwrap()
}

pub fn format_repo(registry: &str, repository: &str) -> String {
    let mut output = String::new();
    if !repository.is_empty() {
        output = repository.to_string();
    }
    if !registry.is_empty() {
        output = format!("{registry}/{output}");
    }

    output
}

pub fn pull_image(
    engine: &docker::Engine,
    image: &str,
    msg_info: MessageInfo,
) -> cross::Result<()> {
    let mut command = docker::subcommand(engine, "pull");
    command.arg(image);
    let out = command.run_and_get_output(msg_info)?;
    command.status_result(msg_info, out.status, Some(&out))?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImageTarget {
    pub triplet: String,
    pub sub: Option<String>,
}

impl ImageTarget {
    pub fn image_name(&self, repository: &str, tag: &str) -> String {
        if let Some(sub) = &self.sub {
            format!("{repository}/{}:{tag}-{sub}", self.triplet)
        } else {
            format!("{repository}/{}:{tag}", self.triplet)
        }
    }

    pub fn alt(&self) -> String {
        if let Some(sub) = &self.sub {
            format!("{}:{sub}", self.triplet,)
        } else {
            self.triplet.to_string()
        }
    }

    /// Determines if this target has a ci image
    pub fn has_ci_image(&self) -> bool {
        let matrix = get_matrix();
        matrix
            .iter()
            .any(|m| m.builds_image() && m.target == self.triplet && m.sub == self.sub)
    }

    /// Determine if this target uses the default test script
    pub fn is_default_test_image(&self) -> bool {
        self.triplet != "cross"
    }

    /// Determine if this target needs to interact with the project root.
    pub fn needs_workspace_root_context(&self) -> bool {
        self.triplet == "cross"
    }
}

impl std::str::FromStr for ImageTarget {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((target, sub)) = s.split_once('.') {
            Ok(ImageTarget {
                triplet: target.to_string(),
                sub: Some(sub.to_string()),
            })
        } else {
            Ok(ImageTarget {
                triplet: s.to_string(),
                sub: None,
            })
        }
    }
}

impl std::fmt::Display for ImageTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(sub) = &self.sub {
            write!(f, "{}.{sub}", self.triplet,)
        } else {
            write!(f, "{}", self.triplet)
        }
    }
}

pub fn has_nightly(msg_info: MessageInfo) -> cross::Result<bool> {
    cross::cargo_command()
        .arg("+nightly")
        .run_and_get_output(msg_info)
        .map(|o| o.status.success())
        .map_err(Into::into)
}

pub fn get_channel_prefer_nightly(
    msg_info: MessageInfo,
    toolchain: Option<&str>,
) -> cross::Result<Option<&str>> {
    Ok(match toolchain {
        Some(t) => Some(t),
        None => match has_nightly(msg_info)? {
            true => Some("nightly"),
            false => None,
        },
    })
}

pub fn cargo(channel: Option<&str>) -> Command {
    let mut command = cross::cargo_command();
    if let Some(channel) = channel {
        command.arg(&format!("+{channel}"));
    }
    command
}

pub fn cargo_metadata(msg_info: MessageInfo) -> cross::Result<cross::CargoMetadata> {
    cross::cargo_metadata_with_args(Some(Path::new(env!("CARGO_MANIFEST_DIR"))), None, msg_info)?
        .ok_or_else(|| eyre::eyre!("could not find cross workspace"))
}

pub fn project_dir(msg_info: MessageInfo) -> cross::Result<PathBuf> {
    Ok(cargo_metadata(msg_info)?.workspace_root)
}

// note: for GHA actions we need to output these tags no matter the verbosity level
pub fn gha_print(content: &str) {
    println!("{}", content)
}

// note: for GHA actions we need to output these tags no matter the verbosity level
pub fn gha_error(content: &str) {
    println!("::error {}", content)
}

#[track_caller]
pub fn gha_output(tag: &str, content: &str) {
    if content.contains('\n') {
        // https://github.com/actions/toolkit/issues/403
        panic!("output `{tag}` contains newlines, consider serializing with json and deserializing in gha with fromJSON()")
    }
    println!("::set-output name={tag}::{}", content)
}

pub fn read_dockerfiles(msg_info: MessageInfo) -> cross::Result<Vec<(PathBuf, String)>> {
    let root = project_dir(msg_info)?;
    let docker = root.join("docker");
    let mut dockerfiles = vec![];
    for entry in fs::read_dir(docker)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let file_name = entry.file_name();
        if file_type.is_file() && file_name.to_utf8()?.starts_with("Dockerfile") {
            let contents = fs::read_to_string(entry.path())?;
            dockerfiles.push((entry.path().to_path_buf(), contents));
        }
    }

    Ok(dockerfiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    use cross::shell::Verbosity;
    use std::collections::BTreeMap;

    #[test]
    fn check_ubuntu_base() -> cross::Result<()> {
        // count all the entries of FROM for our images
        let mut counts = BTreeMap::new();
        let dockerfiles = read_dockerfiles(Verbosity::Verbose.into())?;
        for (path, dockerfile) in dockerfiles {
            let lines: Vec<&str> = dockerfile.lines().collect();
            let index = lines
                .iter()
                .map(|x| x.trim())
                .position(|x| x.to_lowercase().starts_with("from"))
                .ok_or_else(|| eyre::eyre!("unable to find FROM instruction for {:?}", path))?;
            let tag = lines[index]
                .split_whitespace()
                .nth(1)
                .ok_or_else(|| eyre::eyre!("invalid FROM instruction, got {}", lines[index]))?;
            if let Some(value) = counts.get_mut(tag) {
                *value += 1;
            } else {
                counts.insert(tag.to_string(), 1);
            }
        }

        // Now, get the most common and ensure our base is correct.
        let actual_base = cross::docker::UBUNTU_BASE;
        let max_base = counts
            .iter()
            .max_by(|x, y| x.1.cmp(y.1))
            .map(|(k, _)| k)
            .ok_or_else(|| eyre::eyre!("have no dockerfiles"))?;

        if actual_base != max_base {
            eyre::bail!("most common base image is {max_base} but source code has {actual_base}")
        } else {
            Ok(())
        }
    }
}

pub fn write_to_string(path: &Path, contents: &str) -> cross::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)?;
    writeln!(file, "{}", contents)?;
    Ok(())
}
