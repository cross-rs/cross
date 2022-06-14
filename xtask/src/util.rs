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
    #[serde(default)]
    pub run: i64,
    pub os: String,
}

impl Matrix {
    pub fn has_test(&self, target: &str) -> bool {
        // bare-metal targets don't have unittests right now
        self.run != 0 && !target.contains("-none-")
    }
}

pub fn get_matrix() -> cross::Result<Vec<Matrix>> {
    let workflow: Workflow = serde_yaml::from_str(WORKFLOW)?;
    let matrix = &workflow.jobs.generate_matrix.steps[0].env.matrix;
    serde_yaml::from_str(matrix).map_err(Into::into)
}
