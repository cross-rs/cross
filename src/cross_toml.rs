#![doc = include_str!("../docs/cross_toml.md")]

use crate::errors::*;
use crate::{Target, TargetList};
use serde::Deserialize;
use std::collections::HashMap;

/// Build environment configuration
#[derive(Debug, Deserialize, PartialEq)]
pub struct CrossBuildEnvConfig {
    #[serde(default)]
    volumes: Vec<String>,
    #[serde(default)]
    passthrough: Vec<String>,
}

/// Build configuration
#[derive(Debug, Deserialize, PartialEq, Default)]
pub struct CrossBuildConfig {
    env: Option<CrossBuildEnvConfig>,
    xargo: Option<bool>,
    target: Option<String>,
}

/// Target configuration
#[derive(Debug, Deserialize, PartialEq)]
pub struct CrossTargetConfig {
    #[serde(default)]
    passthrough: Vec<String>,
    #[serde(default)]
    volumes: Vec<String>,
    xargo: Option<bool>,
    image: Option<String>,
    runner: Option<String>,
}

/// Cross configuration
#[derive(Debug, Deserialize, PartialEq)]
pub struct CrossToml {
    #[serde(default, rename = "target")]
    targets: HashMap<Target, CrossTargetConfig>,
    build: Option<CrossBuildConfig>,
}

impl CrossToml {
    /// Parses the [`CrossToml`] from a string
    pub fn from_str(toml_str: &str) -> Result<Self> {
        let cfg: CrossToml = toml::from_str(toml_str)?;
        Ok(cfg)
    }

    /// Returns the `target.{}.image` part of `Cross.toml`
    pub fn image(&self, target: &Target) -> Option<String> {
        self.get_target(target).and_then(|t| t.image.clone())
    }

    /// Returns the `target.{}.runner` part of `Cross.toml`
    pub fn runner(&self, target: &Target) -> Option<String> {
        self.get_target(target).and_then(|t| t.runner.clone())
    }

    /// Returns the `build.xargo` or the `target.{}.xargo` part of `Cross.toml`
    pub fn xargo(&self, target: &Target) -> (Option<bool>, Option<bool>) {
        let build_xargo = self.build.as_ref().and_then(|b| b.xargo);
        let target_xargo = self.get_target(target).and_then(|t| t.xargo);

        (build_xargo, target_xargo)
    }

    /// Returns the list of environment variables to pass through for `build`,
    pub fn env_passthrough_build(&self) -> Vec<String> {
        self.get_build_env()
            .map_or(Vec::new(), |e| e.passthrough.clone())
    }

    /// Returns the list of environment variables to pass through for `target`,
    pub fn env_passthrough_target(&self, target: &Target) -> Vec<String> {
        self.get_target(target)
            .map_or(Vec::new(), |t| t.passthrough.clone())
    }

    /// Returns the list of environment variables to pass through for `build`,
    pub fn env_volumes_build(&self) -> Vec<String> {
        self.get_build_env()
            .map_or(Vec::new(), |e| e.volumes.clone())
    }

    /// Returns the list of environment variables to pass through for `target`,
    pub fn env_volumes_target(&self, target: &Target) -> Vec<String> {
        self.get_target(target)
            .map_or(Vec::new(), |t| t.volumes.clone())
    }

    /// Returns the default target to build,
    pub fn target(&self, target_list: &TargetList) -> Option<Target> {
        self.build.as_ref().and_then(|b| b.target.as_ref()).map(|t| Target::from(t, target_list))
    }

    /// Returns a reference to the [`CrossTargetConfig`] of a specific `target`
    fn get_target(&self, target: &Target) -> Option<&CrossTargetConfig> {
        self.targets.get(target)
    }

    /// Returns a reference to the [`CrossBuildEnvConfig`]
    fn get_build_env(&self) -> Option<&CrossBuildEnvConfig> {
        self.build.as_ref().and_then(|b| b.env.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn parse_empty_toml() -> Result<()> {
        let cfg = CrossToml {
            targets: HashMap::new(),
            build: None,
        };
        let parsed_cfg = CrossToml::from_str("")?;

        assert_eq!(parsed_cfg, cfg);

        Ok(())
    }

    #[test]
    pub fn parse_build_toml() -> Result<()> {
        let cfg = CrossToml {
            targets: HashMap::new(),
            build: Some(CrossBuildConfig {
                env: Some(CrossBuildEnvConfig {
                    volumes: vec!["VOL1_ARG".to_string(), "VOL2_ARG".to_string()],
                    passthrough: vec!["VAR1".to_string(), "VAR2".to_string()],
                }),
                xargo: Some(true),
                target: None,
            }),
        };

        let test_str = r#"
          [build]
          xargo = true

          [build.env]
          volumes = ["VOL1_ARG", "VOL2_ARG"]
          passthrough = ["VAR1", "VAR2"]
        "#;
        let parsed_cfg = CrossToml::from_str(test_str)?;

        assert_eq!(parsed_cfg, cfg);

        Ok(())
    }

    #[test]
    pub fn parse_target_toml() -> Result<()> {
        let mut target_map = HashMap::new();
        target_map.insert(
            Target::BuiltIn {
                triple: "aarch64-unknown-linux-gnu".to_string(),
            },
            CrossTargetConfig {
                passthrough: vec!["VAR1".to_string(), "VAR2".to_string()],
                volumes: vec!["VOL1_ARG".to_string(), "VOL2_ARG".to_string()],
                xargo: Some(false),
                image: Some("test-image".to_string()),
                runner: None,
            },
        );

        let cfg = CrossToml {
            targets: target_map,
            build: None,
        };

        let test_str = r#"
          [target.aarch64-unknown-linux-gnu]
          volumes = ["VOL1_ARG", "VOL2_ARG"]
          passthrough = ["VAR1", "VAR2"]
          xargo = false
          image = "test-image"
        "#;
        let parsed_cfg = CrossToml::from_str(test_str)?;

        assert_eq!(parsed_cfg, cfg);

        Ok(())
    }
}
