//! Implements the parsing of `Cross.toml`
//!
//! `Cross.toml` can contain the following elements:
//!
//! # `build`
//! The `build` key allows you to set global variables, e.g.:
//!
//! ```toml
//! [build]
//! xargo = true
//! ```
//!
//! # `build.env`
//! With the `build.env` key you can globally set volumes that should be mounted
//! in the Docker container or environment variables that should be passed through.
//! For example:
//!
//! ```toml
//! [build.env]
//! volumes = ["vol1", "vol2"]
//! passthrough = ["IMPORTANT_ENV_VARIABLES"]
//! ```
//!
//! # `target.TARGET`
//! The `target` key allows you to specify parameters for specific compilation targets.
//!
//! ```toml
//! [target.aarch64-unknown-linux-gnu]
//! volumes = ["vol1", "vol2"]
//! passthrough = ["VAR1", "VAR2"]
//! xargo = false
//! image = "test-image"
//! runner = "custom-runner"
//! ```
//! 

use std::collections::HashMap;
use serde::Deserialize;
use crate::errors::*;
use crate::Target;

/// Build environment configuration
#[derive(Debug, Deserialize, PartialEq)]
pub struct CrossBuildEnvConfig {
    volumes: Option<Vec<String>>,
    passthrough: Option<Vec<String>>,
}

/// Build configuration
#[derive(Debug, Deserialize, PartialEq)]
pub struct CrossBuildConfig {
    env: Option<CrossBuildEnvConfig>,
    xargo: Option<bool>,
    target: Option<String>,
}

/// Target configuration
#[derive(Debug, Deserialize, PartialEq)]
pub struct CrossTargetConfig {
    passthrough: Option<Vec<String>>,
    volumes: Option<Vec<String>>,
    xargo: Option<bool>,
    image: Option<String>,
    runner: Option<String>,
}

/// Wrapper struct for `Target` -> `CrossTargetConfig` mappings
///
/// This is used to circumvent that serde's flatten and field aliases
/// currently don't work together: <https://github.com/serde-rs/serde/issues/1504>
#[derive(Debug, Deserialize, PartialEq)]
pub struct CrossTargetMapConfig {
    #[serde(flatten)]
    inner: HashMap<Target, CrossTargetConfig>,
}

/// Cross configuration 
#[derive(Debug, Deserialize, PartialEq)]
pub struct CrossToml {
    #[serde(rename = "target")]
    targets: Option<CrossTargetMapConfig>,
    build: Option<CrossBuildConfig>,
}

impl CrossToml {
    /// Parses the `CrossConfig` from a string
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
           .and_then(|e| e.passthrough.as_ref())
           .map_or(Vec::new(), |v| v.to_vec())
    }

    /// Returns the list of environment variables to pass through for `target`,
    pub fn env_passthrough_target(&self, target: &Target) -> Vec<String> {
        self.get_target(target)
            .and_then(|t| t.passthrough.as_ref())
            .map_or(Vec::new(), |v| v.to_vec())
    }

    /// Returns the list of environment variables to pass through for `build`,
    pub fn env_volumes_build(&self) -> Vec<String> {
        self.get_build_env()
            .and_then(|e| e.volumes.as_ref())
            .map_or(Vec::new(), |v| v.to_vec())
    }

    /// Returns the list of environment variables to pass through for `target`,
    pub fn env_volumes_target(&self, target: &Target) -> Vec<String> {
        self.get_target(target)
            .and_then(|t| t.volumes.as_ref())
            .map_or(Vec::new(), |v| v.to_vec())
    }

    /// Returns a reference to the `CrossTargetConfig` of a specific `target`
    fn get_target(&self, target: &Target) -> Option<&CrossTargetConfig> {
        self.targets.as_ref().and_then(|t| t.inner.get(target))
    }

    /// Returns a reference to the `CrossBuildEnvConfig`
    fn get_build_env(&self) -> Option<&CrossBuildEnvConfig> {
        self.build.as_ref().and_then(|b| b.env.as_ref())
    }
}

mod tests {
    use super::*;

    #[test]
    pub fn parse_empty_toml() -> Result<()> {
        let cfg = CrossToml { targets: None, build: None };
        let parsed_cfg = CrossToml::from_str("")?;

        assert_eq!(parsed_cfg, cfg);

        Ok(())
    }

    #[test]
    pub fn parse_build_toml() -> Result<()> {
        let cfg = CrossToml {
            targets: None,
            build: Some(CrossBuildConfig {
                env: Some(CrossBuildEnvConfig {
                    volumes: Some(vec!["vol1".to_string(), "vol2".to_string()]),
                    passthrough: Some(vec!["VAR1".to_string(), "VAR2".to_string()])
                }),
                xargo: Some(true),
            })
        };

        let test_str = r#"
          [build]
          xargo = true
 
          [build.env]
          volumes = ["vol1", "vol2"]
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
            Target::BuiltIn { triple: "aarch64-unknown-linux-gnu".to_string() },
            CrossTargetConfig {
                passthrough: Some(vec!["VAR1".to_string(), "VAR2".to_string()]),
                volumes: Some(vec!["vol1".to_string(), "vol2".to_string()]),
                xargo: Some(false),
                image: Some("test-image".to_string()),
                runner: None,
            }
        );

        let cfg = CrossToml {
            targets: Some(CrossTargetMapConfig { inner: target_map }),
            build: None,
        };

        let test_str = r#"
          [target.aarch64-unknown-linux-gnu]
          volumes = ["vol1", "vol2"]
          passthrough = ["VAR1", "VAR2"]
          xargo = false
          image = "test-image"
        "#;
        let parsed_cfg = CrossToml::from_str(test_str)?;

        assert_eq!(parsed_cfg, cfg);

        Ok(())
    }
}
