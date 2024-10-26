use std::path::{Path, PathBuf};
use std::process::Command;

use rustc_version::{Version, VersionMeta};
use serde::Deserialize;

use crate::docker::ImagePlatform;
use crate::errors::*;
use crate::extensions::{env_program, CommandExt};
use crate::shell::MessageInfo;
use crate::TargetTriple;

#[derive(Debug)]
pub struct TargetList {
    pub triples: Vec<String>,
}

impl TargetList {
    #[must_use]
    pub fn contains(&self, triple: &str) -> bool {
        self.triples.iter().any(|t| t == triple)
    }
}

pub trait VersionMetaExt {
    fn host(&self) -> TargetTriple;
    fn needs_interpreter(&self) -> bool;
    fn commit_hash(&self) -> String;
}

impl VersionMetaExt for VersionMeta {
    fn host(&self) -> TargetTriple {
        TargetTriple::from(&*self.host)
    }

    fn needs_interpreter(&self) -> bool {
        self.semver < Version::new(1, 19, 0)
    }

    fn commit_hash(&self) -> String {
        self.commit_hash.as_ref().map_or_else(
            || hash_from_version_string(&self.short_version_string, 2),
            |x| short_commit_hash(x),
        )
    }
}

fn short_commit_hash(hash: &str) -> String {
    // short version hashes are always 9 digits
    //  https://github.com/rust-lang/cargo/pull/10579
    const LENGTH: usize = 9;

    hash.get(..LENGTH)
        .unwrap_or_else(|| panic!("commit hash must be at least {LENGTH} characters long"))
        .to_owned()
}

#[must_use]
pub fn hash_from_version_string(version: &str, index: usize) -> String {
    let is_hash = |x: &str| x.chars().all(|c| c.is_ascii_hexdigit());
    let is_date = |x: &str| x.chars().all(|c| matches!(c, '-' | '0'..='9'));

    // the version can be one of two forms:
    //   multirust channel string: `"1.61.0 (fe5b13d68 2022-05-18)"`
    //   short version string: `"rustc 1.61.0 (fe5b13d68 2022-05-18)"`
    // want to extract the commit hash if we can, if not, just hash the string.
    if let Some((commit, date)) = version
        .splitn(index + 1, ' ')
        .nth(index)
        .and_then(|meta| meta.strip_prefix('('))
        .and_then(|meta| meta.strip_suffix(')'))
        .and_then(|meta| meta.split_once(' '))
    {
        if is_hash(commit) && is_date(date) {
            return short_commit_hash(commit);
        }
    }

    // fallback: can't extract the hash. just create a hash of the version string.
    let buffer = const_sha1::ConstSlice::from_slice(version.as_bytes());
    short_commit_hash(&const_sha1::sha1_from_const_slice(&buffer).to_string())
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct QualifiedToolchain {
    pub channel: String,
    pub date: Option<String>,
    pub(self) host: ImagePlatform,
    pub is_custom: bool,
    pub full: String,
    pub(self) sysroot: PathBuf,
}

impl QualifiedToolchain {
    pub fn new(
        channel: &str,
        date: &Option<String>,
        host: &ImagePlatform,
        sysroot: &Path,
        is_custom: bool,
    ) -> Self {
        let mut this = Self {
            channel: channel.to_owned(),
            date: date.clone(),
            host: host.clone(),
            is_custom,
            full: if let Some(date) = date {
                format!("{}-{}-{}", channel, date, host.target)
            } else {
                format!("{}-{}", channel, host.target)
            },
            sysroot: sysroot.to_owned(),
        };
        if !is_custom {
            this.sysroot.set_file_name(&this.full);
        }
        this
    }

    /// Replace the host, does nothing if ran on a custom toolchain
    pub fn replace_host(&mut self, host: &ImagePlatform) -> &mut Self {
        if !self.is_custom {
            *self = Self::new(&self.channel, &self.date, host, &self.sysroot, false);
            self.sysroot.set_file_name(&self.full);
        }
        self
    }

    /// Makes a good guess as to what the toolchain is compiled to run on.
    pub(crate) fn custom(
        name: &str,
        sysroot: &Path,
        config: &crate::config::Config,
        msg_info: &mut MessageInfo,
    ) -> Result<QualifiedToolchain> {
        if let Some(compat) = config.custom_toolchain_compat() {
            let mut toolchain: QualifiedToolchain = QualifiedToolchain::parse(
                sysroot.to_owned(),
                &compat,
                config,
                msg_info,
            )
            .wrap_err(
                "could not parse CROSS_CUSTOM_TOOLCHAIN_COMPAT as a fully qualified toolchain name",
            )?;
            toolchain.is_custom = true;
            toolchain.full = name.to_owned();
            return Ok(toolchain);
        }
        // a toolchain installed by https://github.com/rust-lang/cargo-bisect-rustc
        if name.starts_with("bisector-nightly") {
            let (_, toolchain) = name.split_once('-').expect("should include -");
            let mut toolchain =
                QualifiedToolchain::parse(sysroot.to_owned(), toolchain, config, msg_info)
                    .wrap_err("could not parse bisector toolchain")?;
            toolchain.is_custom = true;
            toolchain.full = name.to_owned();
            return Ok(toolchain);
        } else if let Ok(stdout) = Command::new(sysroot.join("bin/rustc"))
            .arg("-Vv")
            .run_and_get_stdout(msg_info)
        {
            let rustc_version::VersionMeta {
                build_date,
                channel,
                host,
                ..
            } = rustc_version::version_meta_for(&stdout)?;
            let mut toolchain = QualifiedToolchain::new(
                match channel {
                    rustc_version::Channel::Dev => "dev",
                    rustc_version::Channel::Nightly => "nightly",
                    rustc_version::Channel::Beta => "beta",
                    rustc_version::Channel::Stable => "stable",
                },
                &build_date,
                &ImagePlatform::from_target(host.into())?,
                sysroot,
                true,
            );
            toolchain.full = name.to_owned();
            return Ok(toolchain);
        }
        Err(eyre::eyre!(
            "cross can not figure out what your custom toolchain is"
        ))
        .suggestion("set `CROSS_CUSTOM_TOOLCHAIN_COMPAT` to a fully qualified toolchain name: i.e `nightly-aarch64-unknown-linux-musl`")
    }

    pub fn host(&self) -> &ImagePlatform {
        &self.host
    }

    pub fn get_sysroot(&self) -> &Path {
        &self.sysroot
    }

    /// Grab the current default toolchain
    pub fn default(config: &crate::config::Config, msg_info: &mut MessageInfo) -> Result<Self> {
        let sysroot = sysroot(msg_info)?;

        let default_toolchain_name = sysroot
            .file_name()
            .ok_or_else(|| eyre::eyre!("couldn't get name of active toolchain"))?
            .to_str()
            .ok_or_else(|| eyre::eyre!("toolchain was not utf-8"))?;

        if !config.custom_toolchain() {
            QualifiedToolchain::parse(sysroot.clone(), default_toolchain_name, config, msg_info)
        } else {
            QualifiedToolchain::custom(default_toolchain_name, &sysroot, config, msg_info)
        }
    }

    /// Merge a "picked" toolchain, overriding set fields.
    pub fn with_picked(self, picked: Toolchain) -> Result<Self> {
        let date = picked.date.or(self.date);
        let host = picked
            .host
            .map_or(Ok(self.host), ImagePlatform::from_target)?;
        let channel = picked.channel;

        Ok(QualifiedToolchain::new(
            &channel,
            &date,
            &host,
            &self.sysroot,
            false,
        ))
    }

    pub fn set_sysroot(&mut self, convert: impl Fn(&Path) -> PathBuf) {
        self.sysroot = convert(&self.sysroot);
    }
}

impl std::fmt::Display for QualifiedToolchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.full)
    }
}

impl QualifiedToolchain {
    fn parse(
        sysroot: PathBuf,
        toolchain: &str,
        config: &crate::config::Config,
        msg_info: &mut MessageInfo,
    ) -> Result<Self> {
        match toolchain.parse::<Toolchain>() {
            Ok(Toolchain {
                channel,
                date,
                host: Some(host),
                is_custom,
                full,
            }) => Ok(QualifiedToolchain {
                channel,
                date,
                host: ImagePlatform::from_target(host)?,
                is_custom,
                full,
                sysroot,
            }),
            Ok(_) | Err(_) if config.custom_toolchain() => {
                QualifiedToolchain::custom(toolchain, &sysroot, config, msg_info)
            }
            Ok(_) => Err(eyre::eyre!("toolchain is not fully qualified")
                .with_note(|| "cross expects the toolchain to be a rustup installed toolchain")
                .with_suggestion(|| {
                    "if you're using a custom toolchain try setting `CROSS_CUSTOM_TOOLCHAIN=1` or install rust via rustup"
            })),
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Toolchain {
    pub channel: String,
    pub date: Option<String>,
    pub host: Option<TargetTriple>,
    pub is_custom: bool,
    pub full: String,
}

impl Toolchain {
    pub fn remove_host(&self) -> Self {
        let mut new = Self {
            host: None,
            ..self.clone()
        };
        if let Some(host) = &self.host {
            new.full = new.full.replace(&format!("-{host}"), "");
        }
        new
    }
}

impl std::fmt::Display for Toolchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.full)
    }
}

impl std::str::FromStr for Toolchain {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn dig(s: &str) -> bool {
            s.chars().all(|c: char| c.is_ascii_digit())
        }
        if let Some((channel, parts)) = s.split_once('-') {
            if parts.starts_with(|c: char| c.is_ascii_digit()) {
                // a date, YYYY-MM-DD
                let mut split = parts.splitn(4, '-');
                let ymd = [split.next(), split.next(), split.next()];
                let ymd = match ymd {
                    [Some(y), Some(m), Some(d)] if dig(y) && dig(m) && dig(d) => {
                        format!("{y}-{m}-{d}")
                    }
                    _ => eyre::bail!("invalid toolchain `{s}`"),
                };
                Ok(Toolchain {
                    channel: channel.to_owned(),
                    date: Some(ymd),
                    host: split.next().map(|s| s.into()),
                    is_custom: false,
                    full: s.to_owned(),
                })
            } else {
                // channel-host
                Ok(Toolchain {
                    channel: channel.to_owned(),
                    date: None,
                    host: Some(parts.into()),
                    is_custom: false,
                    full: s.to_owned(),
                })
            }
        } else {
            Ok(Toolchain {
                channel: s.to_owned(),
                date: None,
                host: None,
                is_custom: false,
                full: s.to_owned(),
            })
        }
    }
}

#[must_use]
pub fn rustc_command() -> Command {
    Command::new(env_program("RUSTC", "rustc"))
}

pub fn target_list(msg_info: &mut MessageInfo) -> Result<TargetList> {
    rustc_command()
        .args(["--print", "target-list"])
        .run_and_get_stdout(msg_info)
        .map(|s| TargetList {
            triples: s.lines().map(|l| l.to_owned()).collect(),
        })
}

pub fn sysroot(msg_info: &mut MessageInfo) -> Result<PathBuf> {
    let stdout = rustc_command()
        .args(["--print", "sysroot"])
        .run_and_get_stdout(msg_info)?
        .trim()
        .to_owned();
    Ok(PathBuf::from(stdout))
}

pub fn version_meta() -> Result<rustc_version::VersionMeta> {
    rustc_version::version_meta().wrap_err("couldn't fetch the `rustc` version")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bisect() {
        QualifiedToolchain::custom(
            "bisector-nightly-2022-04-26-x86_64-unknown-linux-gnu",
            "/tmp/cross/sysroot".as_ref(),
            &crate::config::Config::new(None),
            &mut MessageInfo::create(2, false, None).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn hash_from_rustc() {
        assert_eq!(
            hash_from_version_string("1.61.0 (fe5b13d68 2022-05-18)", 1),
            "fe5b13d68"
        );
        assert_eq!(
            hash_from_version_string("rustc 1.61.0 (fe5b13d68 2022-05-18)", 2),
            "fe5b13d68"
        );
    }
}
