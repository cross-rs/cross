//! The `Cross.toml` configuration file.
//!
//! For a detailed user documentation of the file and the contents please refer to the [docs in the
//! repo][1].
//!
//! [1]: https://github.com/cross-rs/cross/blob/main/docs/config_file.md

use crate::config::ConfVal;
use crate::docker::custom::PreBuild;
use crate::docker::PossibleImage;
use crate::shell::MessageInfo;
use crate::{config, errors::*};
use crate::{Target, TargetList};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;

/// Environment configuration
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CrossEnvConfig {
    volumes: Option<Vec<String>>,
    passthrough: Option<Vec<String>>,
}

/// Build configuration
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CrossBuildConfig {
    #[serde(default)]
    env: CrossEnvConfig,
    xargo: Option<bool>,
    build_std: Option<BuildStd>,
    #[serde(default, deserialize_with = "opt_string_bool_or_struct")]
    zig: Option<CrossZigConfig>,
    default_target: Option<String>,
    #[serde(default, deserialize_with = "opt_string_or_string_vec")]
    pre_build: Option<PreBuild>,
    #[serde(default, deserialize_with = "opt_string_or_struct")]
    dockerfile: Option<CrossTargetDockerfileConfig>,
}

/// Target configuration
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct CrossTargetConfig {
    xargo: Option<bool>,
    build_std: Option<BuildStd>,
    #[serde(default, deserialize_with = "opt_string_bool_or_struct")]
    zig: Option<CrossZigConfig>,
    #[serde(default, deserialize_with = "opt_string_or_struct")]
    image: Option<PossibleImage>,
    #[serde(default, deserialize_with = "opt_string_or_struct")]
    dockerfile: Option<CrossTargetDockerfileConfig>,
    #[serde(default, deserialize_with = "opt_string_or_string_vec")]
    pre_build: Option<PreBuild>,
    runner: Option<String>,
    #[serde(default)]
    env: CrossEnvConfig,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum BuildStd {
    Bool(bool),
    Crates(Vec<String>),
}

impl Default for BuildStd {
    fn default() -> Self {
        Self::Bool(false)
    }
}

impl BuildStd {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Bool(enabled) => *enabled,
            Self::Crates(arr) => !arr.is_empty(),
        }
    }
}

/// Dockerfile configuration
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
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
            file: s.to_owned(),
            context: None,
            build_args: None,
        })
    }
}

/// Zig configuration
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct CrossZigConfig {
    enable: Option<bool>,
    version: Option<String>,
    #[serde(default, deserialize_with = "opt_string_or_struct")]
    image: Option<PossibleImage>,
}

impl From<&str> for CrossZigConfig {
    fn from(s: &str) -> CrossZigConfig {
        CrossZigConfig {
            enable: Some(true),
            version: Some(s.to_owned()),
            image: None,
        }
    }
}

impl From<bool> for CrossZigConfig {
    fn from(s: bool) -> CrossZigConfig {
        CrossZigConfig {
            enable: Some(s),
            version: None,
            image: None,
        }
    }
}

impl FromStr for CrossZigConfig {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

/// Cross configuration
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CrossToml {
    #[serde(default, rename = "target")]
    pub targets: HashMap<Target, CrossTargetConfig>,
    #[serde(default)]
    pub build: CrossBuildConfig,
}

impl CrossToml {
    /// Parses the [`CrossToml`] from a string
    pub fn parse_from_cross_str(
        toml_str: &str,
        source: Option<&str>,
        msg_info: &mut MessageInfo,
    ) -> Result<(Self, BTreeSet<String>)> {
        let tomld = toml::Deserializer::new(toml_str);
        Self::parse_from_deserializer(tomld, source, msg_info)
    }

    /// Parses the [`CrossToml`] from a string containing the Cargo.toml contents
    pub fn parse_from_cargo_package_str(
        cargo_toml_str: &str,
        msg_info: &mut MessageInfo,
    ) -> Result<Option<(Self, BTreeSet<String>)>> {
        let cargo_toml: toml::Value = toml::from_str(cargo_toml_str)?;
        let cross_metadata_opt = cargo_toml
            .get("package")
            .and_then(|p| p.get("metadata"))
            .and_then(|m| m.get("cross"));

        if let Some(cross_meta) = cross_metadata_opt {
            Ok(Some(Self::parse_from_deserializer(
                cross_meta.clone(),
                None,
                msg_info,
            )?))
        } else {
            Ok(None)
        }
    }

    /// Parses the [`CrossToml`] from a [`Deserializer`]
    pub fn parse_from_deserializer<'de, D>(
        deserializer: D,
        source: Option<&str>,
        msg_info: &mut MessageInfo,
    ) -> Result<(Self, BTreeSet<String>)>
    where
        D: Deserializer<'de>,
        D::Error: Send + Sync + 'static,
    {
        let mut unused = BTreeSet::new();
        let cfg = serde_ignored::deserialize(deserializer, |path| {
            unused.insert(path.to_string());
        })?;

        if !unused.is_empty() {
            msg_info.warn(format_args!(
                "found unused key(s) in Cross configuration{}:\n > {}",
                source.map(|s| format!(" at {s}")).unwrap_or_default(),
                unused.clone().into_iter().collect::<Vec<_>>().join(", ")
            ))?;
        }

        Ok((cfg, unused))
    }

    /// Merges another [`CrossToml`] into `self` and returns a new merged one
    pub fn merge(self, other: CrossToml) -> Result<CrossToml> {
        type ValueMap = serde_json::Map<String, serde_json::Value>;

        fn to_map<S: Serialize>(s: S) -> Result<ValueMap> {
            if let Some(obj) = serde_json::to_value(s)
                .wrap_err("could not convert CrossToml to serde_json::Value")?
                .as_object()
            {
                Ok(obj.clone())
            } else {
                eyre::bail!("failed to serialize CrossToml as object");
            }
        }

        fn from_map<D: DeserializeOwned>(map: ValueMap) -> Result<D> {
            let value = serde_json::to_value(map)
                .wrap_err("could not convert ValueMap to serde_json::Value")?;
            serde_json::from_value(value)
                .wrap_err("could not deserialize serde_json::Value to CrossToml")
        }

        // merge 2 objects. y has precedence over x.
        fn merge_objects(x: &mut ValueMap, y: &ValueMap) -> Option<()> {
            // we need to iterate over both keys, so we need a full deduplication
            let keys: BTreeSet<String> = x.keys().chain(y.keys()).cloned().collect();
            for key in keys {
                let in_x = x.contains_key(&key);
                let in_y = y.contains_key(&key);
                if !in_x && in_y {
                    let yk = y[&key].clone();
                    x.insert(key, yk);
                    continue;
                } else if !in_y {
                    continue;
                }

                let xk = x.get_mut(&key)?;
                let yk = y.get(&key)?;
                if xk.is_null() && !yk.is_null() {
                    *xk = yk.clone();
                    continue;
                } else if yk.is_null() {
                    continue;
                }

                // now we've filtered out missing keys and optional values
                // all key/value pairs should be same type.
                if xk.is_object() {
                    merge_objects(xk.as_object_mut()?, yk.as_object()?)?;
                } else {
                    *xk = yk.clone();
                }
            }

            Some(())
        }

        // Builds maps of objects
        let mut self_map = to_map(self)?;
        let other_map = to_map(other)?;

        merge_objects(&mut self_map, &other_map).ok_or_else(|| eyre::eyre!("could not merge"))?;
        from_map(self_map)
    }

    /// Returns the `target.{}.image` part of `Cross.toml`
    pub fn image(&self, target: &Target) -> Option<&PossibleImage> {
        self.get_target(target).and_then(|t| t.image.as_ref())
    }

    /// Returns the `{}.dockerfile` or `{}.dockerfile.file` part of `Cross.toml`
    pub fn dockerfile(&self, target: &Target) -> ConfVal<&String> {
        self.get_ref(
            target,
            |b| b.dockerfile.as_ref().map(|c| &c.file),
            |t| t.dockerfile.as_ref().map(|c| &c.file),
        )
    }

    /// Returns the `target.{}.dockerfile.context` part of `Cross.toml`
    pub fn dockerfile_context(&self, target: &Target) -> ConfVal<&String> {
        self.get_ref(
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
    pub fn pre_build(&self, target: &Target) -> ConfVal<&PreBuild> {
        self.get_ref(target, |b| b.pre_build.as_ref(), |t| t.pre_build.as_ref())
    }

    /// Returns the `target.{}.runner` part of `Cross.toml`
    pub fn runner(&self, target: &Target) -> Option<&String> {
        self.get_target(target).and_then(|t| t.runner.as_ref())
    }

    /// Returns the `build.xargo` or the `target.{}.xargo` part of `Cross.toml`
    pub fn xargo(&self, target: &Target) -> ConfVal<bool> {
        self.get_value(target, |b| b.xargo, |t| t.xargo)
    }

    /// Returns the `build.build-std` or the `target.{}.build-std` part of `Cross.toml`
    pub fn build_std(&self, target: &Target) -> ConfVal<&BuildStd> {
        self.get_ref(target, |b| b.build_std.as_ref(), |t| t.build_std.as_ref())
    }

    /// Returns the `{}.zig` or `{}.zig.version` part of `Cross.toml`
    pub fn zig(&self, target: &Target) -> ConfVal<bool> {
        self.get_value(
            target,
            |b| b.zig.as_ref().and_then(|z| z.enable),
            |t| t.zig.as_ref().and_then(|z| z.enable),
        )
    }

    /// Returns the `{}.zig` or `{}.zig.version` part of `Cross.toml`
    pub fn zig_version(&self, target: &Target) -> ConfVal<String> {
        self.get_value(
            target,
            |b| b.zig.as_ref().and_then(|c| c.version.clone()),
            |t| t.zig.as_ref().and_then(|c| c.version.clone()),
        )
    }

    /// Returns the  `{}.zig.image` part of `Cross.toml`
    pub fn zig_image(&self, target: &Target) -> ConfVal<PossibleImage> {
        self.get_value(
            target,
            |b| b.zig.as_ref().and_then(|c| c.image.clone()),
            |t| t.zig.as_ref().and_then(|c| c.image.clone()),
        )
    }

    /// Returns the list of environment variables to pass through for `build` and `target`
    pub fn env_passthrough(&self, target: &Target) -> ConfVal<&[String]> {
        self.get_ref(
            target,
            |build| build.env.passthrough.as_deref(),
            |t| t.env.passthrough.as_deref(),
        )
    }

    /// Returns the list of environment variables to pass through for `build` and `target`
    pub fn env_volumes(&self, target: &Target) -> ConfVal<&[String]> {
        self.get_ref(
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

    fn get_value<T>(
        &self,
        target_triple: &Target,
        get_build: impl Fn(&CrossBuildConfig) -> Option<T>,
        get_target: impl Fn(&CrossTargetConfig) -> Option<T>,
    ) -> ConfVal<T> {
        let build = get_build(&self.build);
        let target = self.get_target(target_triple).and_then(get_target);
        ConfVal::new(build, target)
    }

    fn get_ref<T: ?Sized>(
        &self,
        target_triple: &Target,
        get_build: impl Fn(&CrossBuildConfig) -> Option<&T>,
        get_target: impl Fn(&CrossTargetConfig) -> Option<&T>,
    ) -> ConfVal<&T> {
        let build = get_build(&self.build);
        let target = self.get_target(target_triple).and_then(get_target);
        ConfVal::new(build, target)
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

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData))
}

fn opt_string_or_string_vec<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de> + std::str::FromStr<Err = std::convert::Infallible> + From<Vec<String>>,
    D: serde::Deserializer<'de>,
{
    use std::{fmt, marker::PhantomData};

    use serde::de::{self, SeqAccess, Visitor};

    struct StringOrStringVec<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStringVec<T>
    where
        T: Deserialize<'de> + FromStr<Err = std::convert::Infallible> + From<Vec<String>>,
    {
        type Value = Option<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("string or seq")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(FromStr::from_str(value).ok())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut vec: Vec<String> = vec![];

            while let Some(inner) = seq.next_element::<String>()? {
                vec.push(inner);
            }
            Ok(Some(vec.into()))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(StringOrStringVec(PhantomData))
}

fn opt_string_bool_or_struct<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de> + From<bool> + std::str::FromStr<Err = std::convert::Infallible>,
    D: serde::Deserializer<'de>,
{
    use std::{fmt, marker::PhantomData};

    use serde::de::{self, MapAccess, Visitor};

    struct StringBoolOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringBoolOrStruct<T>
    where
        T: Deserialize<'de> + From<bool> + std::str::FromStr<Err = std::convert::Infallible>,
    {
        type Value = Option<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("string, bool, or map")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(From::from(value)))
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

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(StringBoolOrStruct(PhantomData))
}

#[cfg(test)]
mod tests {
    use crate::docker::{ImagePlatform, ImageReference};

    use super::*;
    use crate::shell;

    macro_rules! m {
        () => {
            MessageInfo::new(shell::ColorChoice::Never, shell::Verbosity::Quiet)
        };
    }

    macro_rules! p {
        ($x:literal) => {
            $x.parse()?
        };
    }

    #[test]
    pub fn parse_empty_toml() -> Result<()> {
        let cfg = CrossToml {
            targets: HashMap::new(),
            build: CrossBuildConfig::default(),
        };
        let (parsed_cfg, unused) = CrossToml::parse_from_cross_str("", None, &mut m!())?;

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
                    volumes: Some(vec![p!("VOL1_ARG"), p!("VOL2_ARG")]),
                    passthrough: Some(vec![p!("VAR1"), p!("VAR2")]),
                },
                xargo: Some(true),
                build_std: None,
                zig: None,
                default_target: None,
                pre_build: Some(PreBuild::Lines(vec![p!("echo 'Hello World!'")])),
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
        let (parsed_cfg, unused) = CrossToml::parse_from_cross_str(test_str, None, &mut m!())?;

        assert_eq!(parsed_cfg, cfg);
        assert!(unused.is_empty());

        Ok(())
    }

    #[test]
    pub fn parse_target_toml() -> Result<()> {
        let mut target_map = HashMap::new();
        target_map.insert(
            Target::BuiltIn {
                triple: "aarch64-unknown-linux-gnu".into(),
            },
            CrossTargetConfig {
                env: CrossEnvConfig {
                    passthrough: Some(vec![p!("VAR1"), p!("VAR2")]),
                    volumes: Some(vec![p!("VOL1_ARG"), p!("VOL2_ARG")]),
                },
                xargo: Some(false),
                build_std: Some(BuildStd::Bool(true)),
                zig: None,
                image: Some("test-image".into()),
                runner: None,
                dockerfile: None,
                pre_build: Some(PreBuild::Lines(vec![])),
            },
        );
        target_map.insert(
            Target::BuiltIn {
                triple: "aarch64-unknown-linux-musl".into(),
            },
            CrossTargetConfig {
                env: CrossEnvConfig {
                    passthrough: None,
                    volumes: None,
                },
                xargo: None,
                build_std: None,
                zig: Some(CrossZigConfig {
                    enable: Some(true),
                    version: Some(p!("2.17")),
                    image: Some("zig:local".into()),
                }),
                image: None,
                runner: None,
                dockerfile: None,
                pre_build: None,
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

            [target.aarch64-unknown-linux-musl.zig]
            enable = true
            version = "2.17"
            image = "zig:local"
        "#;
        let (parsed_cfg, unused) = CrossToml::parse_from_cross_str(test_str, None, &mut m!())?;

        assert_eq!(parsed_cfg, cfg);
        assert!(unused.is_empty());

        Ok(())
    }

    #[test]
    pub fn parse_mixed_toml() -> Result<()> {
        let mut target_map = HashMap::new();
        target_map.insert(
            Target::BuiltIn {
                triple: "aarch64-unknown-linux-gnu".into(),
            },
            CrossTargetConfig {
                xargo: Some(false),
                build_std: None,
                zig: None,
                image: Some(PossibleImage {
                    reference: ImageReference::Name("test-image".to_owned()),
                    toolchain: vec![ImagePlatform::from_target(
                        "aarch64-unknown-linux-musl".into(),
                    )?],
                }),
                dockerfile: Some(CrossTargetDockerfileConfig {
                    file: p!("Dockerfile.test"),
                    context: None,
                    build_args: None,
                }),
                pre_build: Some(PreBuild::Lines(vec![p!("echo 'Hello'")])),
                runner: None,
                env: CrossEnvConfig {
                    passthrough: None,
                    volumes: Some(vec![p!("VOL")]),
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
                zig: Some(CrossZigConfig {
                    enable: None,
                    version: None,
                    image: Some(PossibleImage {
                        reference: ImageReference::Name("zig:local".to_owned()),
                        toolchain: vec![ImagePlatform::from_target(
                            "aarch64-unknown-linux-gnu".into(),
                        )?],
                    }),
                }),
                default_target: None,
                pre_build: Some(PreBuild::Lines(vec![])),
                dockerfile: None,
            },
        };

        let test_str = r#"
            [build]
            xargo = true
            pre-build = []

            [build.zig.image]
            name = "zig:local"
            toolchain = ["aarch64-unknown-linux-gnu"]

            [build.env]
            passthrough = []

            [target.aarch64-unknown-linux-gnu]
            xargo = false
            dockerfile = "Dockerfile.test"
            pre-build = ["echo 'Hello'"]
            image.name = "test-image"
            image.toolchain = ["aarch64-unknown-linux-musl"]

            [target.aarch64-unknown-linux-gnu.env]
            volumes = ["VOL"]
        "#;
        let (parsed_cfg, unused) = CrossToml::parse_from_cross_str(test_str, None, &mut m!())?;

        assert_eq!(parsed_cfg, cfg);
        assert!(unused.is_empty());

        Ok(())
    }

    #[test]
    pub fn parse_from_empty_cargo_toml() -> Result<()> {
        let test_str = r#"
          [package]
          name = "cargo_toml_test_package"
          version = "0.1.0"

          [dependencies]
          cross = "1.2.3"
        "#;

        let res = CrossToml::parse_from_cargo_package_str(test_str, &mut m!())?;
        assert!(res.is_none());

        Ok(())
    }

    #[test]
    pub fn parse_from_cargo_toml() -> Result<()> {
        let cfg = CrossToml {
            targets: HashMap::new(),
            build: CrossBuildConfig {
                env: CrossEnvConfig {
                    passthrough: None,
                    volumes: None,
                },
                build_std: None,
                xargo: Some(true),
                zig: None,
                default_target: None,
                pre_build: None,
                dockerfile: None,
            },
        };

        let test_str = r#"
          [package]
          name = "cargo_toml_test_package"
          version = "0.1.0"

          [dependencies]
          cross = "1.2.3"

          [package.metadata.cross.build]
          xargo = true
        "#;

        if let Some((parsed_cfg, _unused)) =
            CrossToml::parse_from_cargo_package_str(test_str, &mut m!())?
        {
            assert_eq!(parsed_cfg, cfg);
        } else {
            panic!("Parsing result is None");
        }

        Ok(())
    }

    #[test]
    pub fn fully_populated_roundtrip() -> Result<()> {
        let cfg = r#"
            [target.a]
            xargo = false
            build-std = true
            image.name = "local"
            image.toolchain = ["x86_64-unknown-linux-gnu"]
            dockerfile.file = "Dockerfile"
            dockerfile.context = ".."
            pre-build = ["sh"]
            zig = true

            [target.b]
            pre-build = "sh"
            zig = "2.17"
        "#;

        let (cfg, _) = CrossToml::parse_from_cross_str(cfg, None, &mut m!())?;
        serde_json::from_value::<CrossToml>(serde_json::to_value(cfg)?)?;
        Ok(())
    }

    #[test]
    pub fn merge() -> Result<()> {
        let cfg1_str = r#"
            [target.aarch64-unknown-linux-gnu]
            xargo = false
            build-std = true
            image = "test-image1"

            [target.aarch64-unknown-linux-gnu.env]
            volumes = ["VOL1_ARG"]
            passthrough = ["VAR1"]

            [target.target2]
            xargo = false
            build-std = true
            image = "test-image2"

            [target.target2.env]
            volumes = ["VOL2_ARG"]
            passthrough = ["VAR2"]

            [build]
            build-std = true
            xargo = true

            [build.env]
            volumes = []
            passthrough = ["VAR1", "VAR2"]
        "#;

        let cfg2_str = r#"
            [target.target2]
            xargo = false
            build-std = false
            image = "test-image2-precedence"

            [target.target2.env]
            volumes = ["VOL2_ARG_PRECEDENCE"]
            passthrough = ["VAR2_PRECEDENCE"]

            [target.target3]
            xargo = false
            build-std = true
            image = "@sha256:test-image3"

            [target.target3.env]
            volumes = ["VOL3_ARG"]
            passthrough = ["VAR3"]

            [build]
            build-std = ["core", "alloc"]
            xargo = false
            default-target = "aarch64-unknown-linux-gnu"

            [build.env]
            volumes = []
            passthrough = ["VAR3", "VAR4"]

        "#;

        let cfg_expected_str = r#"
            [target.aarch64-unknown-linux-gnu]
            xargo = false
            build-std = true
            image = "test-image1"

            [target.aarch64-unknown-linux-gnu.env]
            volumes = ["VOL1_ARG"]
            passthrough = ["VAR1"]

            [target.target2]
            xargo = false
            build-std = false
            image = "test-image2-precedence"

            [target.target2.env]
            volumes = ["VOL2_ARG_PRECEDENCE"]
            passthrough = ["VAR2_PRECEDENCE"]

            [target.target3]
            xargo = false
            build-std = true
            image = "@sha256:test-image3"

            [target.target3.env]
            volumes = ["VOL3_ARG"]
            passthrough = ["VAR3"]

            [build]
            build-std = ["core", "alloc"]
            xargo = false
            default-target = "aarch64-unknown-linux-gnu"

            [build.env]
            volumes = []
            passthrough = ["VAR3", "VAR4"]
        "#;

        // Parses configs
        let (cfg1, _) = CrossToml::parse_from_cross_str(cfg1_str, None, &mut m!())?;
        let (cfg2, _) = CrossToml::parse_from_cross_str(cfg2_str, None, &mut m!())?;
        let (cfg_expected, _) = CrossToml::parse_from_cross_str(cfg_expected_str, None, &mut m!())?;

        // Merges config and compares
        let cfg_merged = cfg1.merge(cfg2)?;
        assert_eq!(cfg_expected, cfg_merged);

        // need to test individual values. i've broken this down into
        // tests on values for better error reporting
        let build = &cfg_expected.build;
        assert_eq!(
            build.build_std,
            Some(BuildStd::Crates(vec![
                "core".to_owned(),
                "alloc".to_owned()
            ]))
        );
        assert_eq!(build.xargo, Some(false));
        assert_eq!(build.default_target, Some(p!("aarch64-unknown-linux-gnu")));
        assert_eq!(build.pre_build, None);
        assert_eq!(build.dockerfile, None);
        assert_eq!(build.env.passthrough, Some(vec![p!("VAR3"), p!("VAR4")]));
        assert_eq!(build.env.volumes, Some(vec![]));

        let targets = &cfg_expected.targets;
        let aarch64 = &targets[&Target::new_built_in("aarch64-unknown-linux-gnu")];
        assert_eq!(aarch64.build_std, Some(BuildStd::Bool(true)));
        assert_eq!(aarch64.xargo, Some(false));
        assert_eq!(aarch64.image, Some(p!("test-image1")));
        assert_eq!(aarch64.pre_build, None);
        assert_eq!(aarch64.dockerfile, None);
        assert_eq!(aarch64.env.passthrough, Some(vec![p!("VAR1")]));
        assert_eq!(aarch64.env.volumes, Some(vec![p!("VOL1_ARG")]));

        let target2 = &targets[&Target::new_custom("target2")];
        assert_eq!(target2.build_std, Some(BuildStd::Bool(false)));
        assert_eq!(target2.xargo, Some(false));
        assert_eq!(target2.image, Some(p!("test-image2-precedence")));
        assert_eq!(target2.pre_build, None);
        assert_eq!(target2.dockerfile, None);
        assert_eq!(target2.env.passthrough, Some(vec![p!("VAR2_PRECEDENCE")]));
        assert_eq!(target2.env.volumes, Some(vec![p!("VOL2_ARG_PRECEDENCE")]));

        let target3 = &targets[&Target::new_custom("target3")];
        assert_eq!(target3.build_std, Some(BuildStd::Bool(true)));
        assert_eq!(target3.xargo, Some(false));
        assert_eq!(target3.image, Some(p!("@sha256:test-image3")));
        assert_eq!(target3.pre_build, None);
        assert_eq!(target3.dockerfile, None);
        assert_eq!(target3.env.passthrough, Some(vec![p!("VAR3")]));
        assert_eq!(target3.env.volumes, Some(vec![p!("VOL3_ARG")]));

        Ok(())
    }

    #[test]
    fn pre_build_script() -> Result<()> {
        let toml_str = r#"
            [target.aarch64-unknown-linux-gnu]
            pre-build = "./my-script.sh"

            [build]
            pre-build = ["echo Hello World"]
        "#;
        let (toml, unused) = CrossToml::parse_from_cross_str(toml_str, None, &mut m!())?;
        assert!(unused.is_empty());
        assert!(matches!(
            toml.pre_build(&Target::new_built_in("aarch64-unknown-linux-gnu")),
            ConfVal {
                build: Some(&PreBuild::Lines(_)),
                target: Some(&PreBuild::Single { .. }),
            },
        ));
        Ok(())
    }
}
