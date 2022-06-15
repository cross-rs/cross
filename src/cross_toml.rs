#![doc = include_str!("../docs/cross_toml.md")]

use crate::{config, errors::*};
use crate::{Target, TargetList};
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;

/// Environment configuration
#[derive(Debug, Deserialize, PartialEq, Eq, Default)]
pub struct CrossEnvConfig {
    volumes: Option<Vec<String>>,
    passthrough: Option<Vec<String>>,
}

/// Build configuration
#[derive(Debug, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CrossBuildConfig {
    #[serde(default)]
    env: CrossEnvConfig,
    xargo: Option<bool>,
    build_std: Option<bool>,
    default_target: Option<String>,
    pre_build: Option<Vec<String>>,
    #[serde(default, deserialize_with = "opt_string_or_struct")]
    dockerfile: Option<CrossTargetDockerfileConfig>,
}

/// Target configuration
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct CrossTargetConfig {
    xargo: Option<bool>,
    build_std: Option<bool>,
    image: Option<String>,
    #[serde(default, deserialize_with = "opt_string_or_struct")]
    dockerfile: Option<CrossTargetDockerfileConfig>,
    pre_build: Option<Vec<String>>,
    runner: Option<String>,
    #[serde(default)]
    env: CrossEnvConfig,
}

/// Dockerfile configuration
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct CrossTargetDockerfileConfig {
    file: String,
    context: Option<String>,
    build_args: Option<HashMap<String, String>>,
}

impl FromStr for CrossTargetDockerfileConfig {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(CrossTargetDockerfileConfig {
            file: s.to_string(),
            context: None,
            build_args: None,
        })
    }
}

/// Cross configuration
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct CrossToml {
    #[serde(default, rename = "target")]
    pub targets: HashMap<Target, CrossTargetConfig>,
    #[serde(default)]
    pub build: CrossBuildConfig,
}

impl CrossToml {
    /// Parses the [`CrossToml`] from a string
    pub fn parse(toml_str: &str) -> Result<(Self, BTreeSet<String>)> {
        let tomld = &mut toml::Deserializer::new(toml_str);

        let mut unused = BTreeSet::new();

        let cfg = serde_ignored::deserialize(tomld, |path| {
            unused.insert(path.to_string());
        })?;

        if !unused.is_empty() {
            eprintln!(
                "Warning: found unused key(s) in Cross configuration:\n > {}",
                unused.clone().into_iter().collect::<Vec<_>>().join(", ")
            );
        }

        Ok((cfg, unused))
    }

    /// Returns the `target.{}.image` part of `Cross.toml`
    pub fn image(&self, target: &Target) -> Option<String> {
        self.get_string(target, |_| None, |t| t.image.as_ref())
    }

    /// Returns the `{}.dockerfile` or `{}.dockerfile.file` part of `Cross.toml`
    pub fn dockerfile(&self, target: &Target) -> Option<String> {
        self.get_string(
            target,
            |b| b.dockerfile.as_ref().map(|c| &c.file),
            |t| t.dockerfile.as_ref().map(|c| &c.file),
        )
    }

    /// Returns the `target.{}.dockerfile.context` part of `Cross.toml`
    pub fn dockerfile_context(&self, target: &Target) -> Option<String> {
        self.get_string(
            target,
            |b| b.dockerfile.as_ref().and_then(|c| c.context.as_ref()),
            |t| t.dockerfile.as_ref().and_then(|c| c.context.as_ref()),
        )
    }

    /// Returns the `target.{}.dockerfile.build_args` part of `Cross.toml`
    pub fn dockerfile_build_args(&self, target: &Target) -> Option<HashMap<String, String>> {
        let target = self
            .get_target(target)
            .and_then(|t| t.dockerfile.as_ref())
            .and_then(|d| d.build_args.as_ref());

        let build = self
            .build
            .dockerfile
            .as_ref()
            .and_then(|d| d.build_args.as_ref());

        config::opt_merge(target.cloned(), build.cloned())
    }

    /// Returns the `build.dockerfile.pre-build` and `target.{}.dockerfile.pre-build` part of `Cross.toml`
    pub fn pre_build(&self, target: &Target) -> (Option<&[String]>, Option<&[String]>) {
        self.get_vec(
            target,
            |b| b.pre_build.as_deref(),
            |t| t.pre_build.as_deref(),
        )
    }

    /// Returns the `target.{}.runner` part of `Cross.toml`
    pub fn runner(&self, target: &Target) -> Option<String> {
        self.get_string(target, |_| None, |t| t.runner.as_ref())
    }

    /// Returns the `build.xargo` or the `target.{}.xargo` part of `Cross.toml`
    pub fn xargo(&self, target: &Target) -> (Option<bool>, Option<bool>) {
        self.get_bool(target, |b| b.xargo, |t| t.xargo)
    }

    /// Returns the `build.build-std` or the `target.{}.build-std` part of `Cross.toml`
    pub fn build_std(&self, target: &Target) -> (Option<bool>, Option<bool>) {
        self.get_bool(target, |b| b.build_std, |t| t.build_std)
    }

    /// Returns the list of environment variables to pass through for `build` and `target`
    pub fn env_passthrough(&self, target: &Target) -> (Option<&[String]>, Option<&[String]>) {
        self.get_vec(target, |_| None, |t| t.env.passthrough.as_deref())
    }

    /// Returns the list of environment variables to pass through for `build` and `target`
    pub fn env_volumes(&self, target: &Target) -> (Option<&[String]>, Option<&[String]>) {
        self.get_vec(
            target,
            |build| build.env.volumes.as_deref(),
            |t| t.env.volumes.as_deref(),
        )
    }

    /// Returns the default target to build,
    pub fn default_target(&self, target_list: &TargetList) -> Option<Target> {
        self.build
            .default_target
            .as_ref()
            .map(|t| Target::from(t, target_list))
    }

    /// Returns a reference to the [`CrossTargetConfig`] of a specific `target`
    fn get_target(&self, target: &Target) -> Option<&CrossTargetConfig> {
        self.targets.get(target)
    }

    fn get_string<'a>(
        &'a self,
        target: &Target,
        get_build: impl Fn(&'a CrossBuildConfig) -> Option<&'a String>,
        get_target: impl Fn(&'a CrossTargetConfig) -> Option<&'a String>,
    ) -> Option<String> {
        self.get_target(target)
            .and_then(get_target)
            .or_else(|| get_build(&self.build))
            .map(ToOwned::to_owned)
    }

    fn get_bool(
        &self,
        target: &Target,
        get_build: impl Fn(&CrossBuildConfig) -> Option<bool>,
        get_target: impl Fn(&CrossTargetConfig) -> Option<bool>,
    ) -> (Option<bool>, Option<bool>) {
        let build = get_build(&self.build);
        let target = self.get_target(target).and_then(get_target);

        (build, target)
    }

    fn get_vec(
        &self,
        target_triple: &Target,
        build: impl Fn(&CrossBuildConfig) -> Option<&[String]>,
        target: impl Fn(&CrossTargetConfig) -> Option<&[String]>,
    ) -> (Option<&[String]>, Option<&[String]>) {
        let target = if let Some(t) = self.get_target(target_triple) {
            target(t)
        } else {
            None
        };
        (build(&self.build), target)
    }
}

fn opt_string_or_struct<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de> + std::str::FromStr<Err = std::convert::Infallible>,
    D: serde::Deserializer<'de>,
{
    use std::{fmt, marker::PhantomData};

    use serde::de::{self, MapAccess, Visitor};

    struct StringOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr<Err = std::convert::Infallible>,
    {
        type Value = Option<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(FromStr::from_str(value).ok())
        }

        fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let t: Result<T, _> =
                Deserialize::deserialize(de::value::MapAccessDeserializer::new(map));
            t.map(Some)
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn parse_empty_toml() -> Result<()> {
        let cfg = CrossToml {
            targets: HashMap::new(),
            build: CrossBuildConfig::default(),
        };
        let (parsed_cfg, unused) = CrossToml::parse("")?;

        assert_eq!(parsed_cfg, cfg);
        assert!(unused.is_empty());

        Ok(())
    }

    #[test]
    pub fn parse_build_toml() -> Result<()> {
        let cfg = CrossToml {
            targets: HashMap::new(),
            build: CrossBuildConfig {
                env: CrossEnvConfig {
                    volumes: Some(vec!["VOL1_ARG".to_string(), "VOL2_ARG".to_string()]),
                    passthrough: Some(vec!["VAR1".to_string(), "VAR2".to_string()]),
                },
                xargo: Some(true),
                build_std: None,
                default_target: None,
                pre_build: Some(vec!["echo 'Hello World!'".to_string()]),
                dockerfile: None,
            },
        };

        let test_str = r#"
          [build]
          xargo = true
          pre-build = ["echo 'Hello World!'"]

          [build.env]
          volumes = ["VOL1_ARG", "VOL2_ARG"]
          passthrough = ["VAR1", "VAR2"]
        "#;
        let (parsed_cfg, unused) = CrossToml::parse(test_str)?;

        assert_eq!(parsed_cfg, cfg);
        assert!(unused.is_empty());

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
                env: CrossEnvConfig {
                    passthrough: Some(vec!["VAR1".to_string(), "VAR2".to_string()]),
                    volumes: Some(vec!["VOL1_ARG".to_string(), "VOL2_ARG".to_string()]),
                },
                xargo: Some(false),
                build_std: Some(true),
                image: Some("test-image".to_string()),
                runner: None,
                dockerfile: None,
                pre_build: Some(vec![]),
            },
        );

        let cfg = CrossToml {
            targets: target_map,
            build: CrossBuildConfig::default(),
        };

        let test_str = r#"
            [target.aarch64-unknown-linux-gnu.env]
            volumes = ["VOL1_ARG", "VOL2_ARG"]
            passthrough = ["VAR1", "VAR2"]
            [target.aarch64-unknown-linux-gnu]
            xargo = false
            build-std = true
            image = "test-image"
            pre-build = []
        "#;
        let (parsed_cfg, unused) = CrossToml::parse(test_str)?;

        assert_eq!(parsed_cfg, cfg);
        assert!(unused.is_empty());

        Ok(())
    }

    #[test]
    pub fn parse_mixed_toml() -> Result<()> {
        let mut target_map = HashMap::new();
        target_map.insert(
            Target::BuiltIn {
                triple: "aarch64-unknown-linux-gnu".to_string(),
            },
            CrossTargetConfig {
                xargo: Some(false),
                build_std: None,
                image: None,
                dockerfile: Some(CrossTargetDockerfileConfig {
                    file: "Dockerfile.test".to_string(),
                    context: None,
                    build_args: None,
                }),
                pre_build: Some(vec!["echo 'Hello'".to_string()]),
                runner: None,
                env: CrossEnvConfig {
                    passthrough: None,
                    volumes: Some(vec!["VOL".to_string()]),
                },
            },
        );

        let cfg = CrossToml {
            targets: target_map,
            build: CrossBuildConfig {
                env: CrossEnvConfig {
                    volumes: None,
                    passthrough: Some(vec![]),
                },
                xargo: Some(true),
                build_std: None,
                default_target: None,
                pre_build: Some(vec![]),
                dockerfile: None,
            },
        };

        let test_str = r#"
            [build]
            xargo = true
            pre-build = []

            [build.env]
            passthrough = []

            [target.aarch64-unknown-linux-gnu]
            xargo = false
            dockerfile = "Dockerfile.test"
            pre-build = ["echo 'Hello'"]

            [target.aarch64-unknown-linux-gnu.env]
            volumes = ["VOL"]
        "#;
        let (parsed_cfg, unused) = CrossToml::parse(test_str)?;

        assert_eq!(parsed_cfg, cfg);
        assert!(unused.is_empty());

        Ok(())
    }
}
