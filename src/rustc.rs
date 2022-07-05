use std::path::PathBuf;
use std::process::Command;

use rustc_version::{Version, VersionMeta};

use crate::errors::*;
use crate::extensions::{env_program, CommandExt};
use crate::shell::MessageInfo;
use crate::{Host, Target};

#[derive(Debug)]
pub struct TargetList {
    pub triples: Vec<String>,
}

impl TargetList {
    pub fn contains(&self, triple: &str) -> bool {
        self.triples.iter().any(|t| t == triple)
    }
}

pub trait VersionMetaExt {
    fn host(&self) -> Host;
    fn needs_interpreter(&self) -> bool;
    fn commit_hash(&self) -> String;
}

impl VersionMetaExt for VersionMeta {
    fn host(&self) -> Host {
        Host::from(&*self.host)
    }

    fn needs_interpreter(&self) -> bool {
        self.semver < Version::new(1, 19, 0)
    }

    fn commit_hash(&self) -> String {
        self.commit_hash
            .as_ref()
            .map(|x| short_commit_hash(x))
            .unwrap_or_else(|| hash_from_version_string(&self.short_version_string, 2))
    }
}

fn short_commit_hash(hash: &str) -> String {
    // short version hashes are always 9 digits
    //  https://github.com/rust-lang/cargo/pull/10579
    const LENGTH: usize = 9;

    hash.get(..LENGTH)
        .unwrap_or_else(|| panic!("commit hash must be at least {LENGTH} characters long"))
        .to_string()
}

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
    let buffer = const_sha1::ConstBuffer::from_slice(version.as_bytes());
    short_commit_hash(&const_sha1::sha1(&buffer).to_string())
}

pub fn rustc_command() -> Command {
    Command::new(env_program("RUSTC", "rustc"))
}

pub fn target_list(msg_info: &mut MessageInfo) -> Result<TargetList> {
    rustc_command()
        .args(&["--print", "target-list"])
        .run_and_get_stdout(msg_info)
        .map(|s| TargetList {
            triples: s.lines().map(|l| l.to_owned()).collect(),
        })
}

pub fn sysroot(host: &Host, target: &Target, msg_info: &mut MessageInfo) -> Result<PathBuf> {
    let mut stdout = rustc_command()
        .args(&["--print", "sysroot"])
        .run_and_get_stdout(msg_info)?
        .trim()
        .to_owned();

    // On hosts other than Linux, specify the correct toolchain path.
    if host != &Host::X86_64UnknownLinuxGnu && target.needs_docker() {
        stdout = stdout.replacen(host.triple(), Host::X86_64UnknownLinuxGnu.triple(), 1);
    }

    Ok(PathBuf::from(stdout))
}

pub fn get_sysroot(
    host: &Host,
    target: &Target,
    channel: Option<&str>,
    msg_info: &mut MessageInfo,
) -> Result<(String, PathBuf)> {
    let mut sysroot = sysroot(host, target, msg_info)?;
    let default_toolchain = sysroot
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .ok_or_else(|| eyre::eyre!("couldn't get toolchain name"))?;
    let toolchain = if let Some(channel) = channel {
        [channel]
            .iter()
            .cloned()
            .chain(default_toolchain.splitn(2, '-').skip(1))
            .collect::<Vec<_>>()
            .join("-")
    } else {
        default_toolchain.to_string()
    };
    sysroot.set_file_name(&toolchain);

    Ok((toolchain, sysroot))
}

pub fn version_meta() -> Result<rustc_version::VersionMeta> {
    rustc_version::version_meta().wrap_err("couldn't fetch the `rustc` version")
}

#[cfg(test)]
mod tests {
    use super::*;

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
