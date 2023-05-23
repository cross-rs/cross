use std::str::FromStr;

use clap::Args;
use cross::docker;
use cross::shell::MessageInfo;

#[derive(Args, Debug)]
pub struct ListTargets {
    /// Format version
    #[clap(long)]
    format_version: Option<FormatVersion>,
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub enum FormatVersion {
    #[serde(rename = "1")]
    One,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid format version")]
pub struct FormatVersionError;

impl FromStr for FormatVersion {
    type Err = FormatVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" => Ok(FormatVersion::One),
            _ => Err(FormatVersionError),
        }
    }
}

#[derive(serde::Serialize)]
pub struct Output {
    format_version: FormatVersion,
    #[serde(flatten)]
    other: serde_json::Value,
}

impl ListTargets {
    pub fn verbose(&self) -> bool {
        false
    }

    pub fn quiet(&self) -> bool {
        false
    }

    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    pub fn run(self, msg_info: &mut MessageInfo) -> cross::Result<()> {
        let toml = if let Some(metadata) = cross::cargo_metadata_with_args(None, None, msg_info)? {
            cross::toml(&metadata, msg_info)?
        } else {
            None
        };

        let config = cross::config::Config::new(toml);
        let version = if let Some(version) = self.format_version {
            version
        } else {
            msg_info.warn(
                "please specify `--format-version` flag explicitly to avoid compatibility problems",
            )?;
            FormatVersion::One
        };
        let data = match version {
            FormatVersion::One => self.run_v1(&config, msg_info)?,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&Output {
                format_version: version,
                other: data,
            })?
        );
        Ok(())
    }

    pub fn run_v1(
        self,
        config: &cross::config::Config,
        _msg_info: &mut MessageInfo,
    ) -> cross::Result<serde_json::Value> {
        #[derive(serde::Serialize)]
        struct Target {
            triplet: String,
            platforms: Vec<String>,
        }
        let mut targets: Vec<_> = cross::docker::PROVIDED_IMAGES
            .iter()
            .filter_map(|i| {
                Some(Target {
                    triplet: Some(i.name).filter(|i| *i != "zig")?.to_owned(),
                    platforms: i.platforms.iter().map(ToString::to_string).collect(),
                })
            })
            .collect();
        if let Some(toml_targets) = config.targets() {
            for (target, config) in toml_targets {
                if targets.iter().any(|t| t.triplet == target.triple()) {
                    continue;
                }
                targets.push(Target {
                    triplet: target.triple().to_owned(),
                    platforms: config
                        .image
                        .as_ref()
                        .map(|i| {
                            i.toolchain
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                        })
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| vec![docker::ImagePlatform::DEFAULT.to_string()]),
                })
            }
        }
        Ok(serde_json::json!({
            "targets": targets,
        }))
    }
}
