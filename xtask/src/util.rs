use cross::{docker, CommandExt};
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

pub fn pull_image(engine: &docker::Engine, image: &str, verbose: bool) -> cross::Result<()> {
    let mut command = docker::subcommand(engine, "pull");
    command.arg(image);
    let out = command.run_and_get_output(verbose)?;
    command.status_result(verbose, out.status, Some(&out))?;
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
