use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::bool_from_envvar;
use crate::extensions::CommandExt;
use crate::shell::MessageInfo;
use crate::{errors::*, OutputExt};

use super::{Architecture, ContainerOs};

pub const DOCKER: &str = "docker";
pub const PODMAN: &str = "podman";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EngineType {
    Docker,
    Podman,
    PodmanRemote,
    Other,
}

impl EngineType {
    /// Returns `true` if the engine type is [`Podman`](Self::Podman) or [`PodmanRemote`](Self::PodmanRemote).
    #[must_use]
    pub fn is_podman(&self) -> bool {
        matches!(self, Self::Podman | Self::PodmanRemote)
    }

    /// Returns `true` if the engine type is [`Docker`](EngineType::Docker).
    #[must_use]
    pub fn is_docker(&self) -> bool {
        matches!(self, Self::Docker)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engine {
    pub kind: EngineType,
    pub path: PathBuf,
    pub in_docker: bool,
    pub arch: Option<Architecture>,
    pub os: Option<ContainerOs>,
    pub is_remote: bool,
}

impl Engine {
    pub fn new(
        in_docker: Option<bool>,
        is_remote: Option<bool>,
        msg_info: &mut MessageInfo,
    ) -> Result<Engine> {
        #[allow(clippy::map_err_ignore)]
        let path = get_container_engine()
            .map_err(|_| eyre::eyre!("no container engine found"))
            .with_suggestion(|| "is docker or podman installed?")?;
        Self::from_path(path, in_docker, is_remote, msg_info)
    }

    pub fn from_path(
        path: PathBuf,
        in_docker: Option<bool>,
        is_remote: Option<bool>,
        msg_info: &mut MessageInfo,
    ) -> Result<Engine> {
        let in_docker = match in_docker {
            Some(v) => v,
            None => Self::in_docker(msg_info)?,
        };
        let (kind, arch, os) = get_engine_info(&path, msg_info)?;
        let is_remote = is_remote.unwrap_or_else(Self::is_remote);
        Ok(Engine {
            path,
            kind,
            in_docker,
            arch,
            os,
            is_remote,
        })
    }

    #[must_use]
    pub fn needs_remote(&self) -> bool {
        self.is_remote && self.kind == EngineType::Podman
    }

    pub fn in_docker(msg_info: &mut MessageInfo) -> Result<bool> {
        Ok(
            if let Ok(value) = env::var("CROSS_CONTAINER_IN_CONTAINER") {
                if env::var("CROSS_DOCKER_IN_DOCKER").is_ok() {
                    msg_info.warn(
                        "using both `CROSS_CONTAINER_IN_CONTAINER` and `CROSS_DOCKER_IN_DOCKER`.",
                    )?;
                }
                bool_from_envvar(&value)
            } else if let Ok(value) = env::var("CROSS_DOCKER_IN_DOCKER") {
                // FIXME: remove this when we deprecate CROSS_DOCKER_IN_DOCKER.
                bool_from_envvar(&value)
            } else {
                false
            },
        )
    }

    #[must_use]
    pub fn is_remote() -> bool {
        env::var("CROSS_REMOTE")
            .map(|s| bool_from_envvar(&s))
            .unwrap_or_default()
    }
}

// determine if the container engine is docker. this fixes issues with
// any aliases (#530), and doesn't fail if an executable suffix exists.
fn get_engine_info(
    ce: &Path,
    msg_info: &mut MessageInfo,
) -> Result<(EngineType, Option<Architecture>, Option<ContainerOs>)> {
    let stdout_help = Command::new(ce)
        .arg("--help")
        .run_and_get_stdout(msg_info)?
        .to_lowercase();

    let kind = if stdout_help.contains("podman-remote") {
        EngineType::PodmanRemote
    } else if stdout_help.contains("podman") {
        EngineType::Podman
    } else if stdout_help.contains("docker") && !stdout_help.contains("emulate") {
        EngineType::Docker
    } else {
        EngineType::Other
    };

    // this can fail: podman can give partial output
    //   linux,,,Error: template: version:1:15: executing "version" at <.Arch>:
    //   can't evaluate field Arch in type *define.Version
    let os_arch_server = engine_info(
        ce,
        &["version", "-f", "{{ .Server.Os }},,,{{ .Server.Arch }}"],
        ",,,",
        msg_info,
    );

    let (os_arch_other, os_arch_server_result) = match os_arch_server {
        Ok(Some(os_arch)) => (Ok(Some(os_arch)), None),
        result => {
            if kind.is_podman() {
                (get_podman_info(ce, msg_info), result.err())
            } else {
                (get_custom_info(ce, msg_info), result.err())
            }
        }
    };

    let os_arch = match (os_arch_other, os_arch_server_result) {
        (Ok(os_arch), _) => os_arch,
        (Err(e), Some(server_err)) => return Err(server_err.to_section_report().with_error(|| e)),
        (Err(e), None) => return Err(e.to_section_report()),
    };

    let (os, arch) = os_arch.map_or(<_>::default(), |(os, arch)| (Some(os), Some(arch)));
    Ok((kind, arch, os))
}

#[derive(Debug, thiserror::Error)]
pub enum EngineInfoError {
    #[error(transparent)]
    Eyre(eyre::Report),
    #[error("could not get os and arch")]
    CommandError(#[from] CommandError),
}

impl EngineInfoError {
    pub fn to_section_report(self) -> eyre::Report {
        match self {
            EngineInfoError::Eyre(e) => e,
            EngineInfoError::CommandError(e) => {
                e.to_section_report().wrap_err("could not get os and arch")
            }
        }
    }
}

/// Get engine info
fn engine_info(
    ce: &Path,
    args: &[&str],
    sep: &str,
    msg_info: &mut MessageInfo,
) -> Result<Option<(ContainerOs, Architecture)>, EngineInfoError> {
    let mut cmd = Command::new(ce);
    cmd.args(args);
    let out = cmd
        .run_and_get_output(msg_info)
        .map_err(EngineInfoError::Eyre)?;

    cmd.status_result(msg_info, out.status, Some(&out))?;

    out.stdout()?
        .to_lowercase()
        .trim()
        .split_once(sep)
        .map(|(os, arch)| -> Result<_> { Ok((ContainerOs::new(os)?, Architecture::new(arch)?)) })
        .transpose()
        .map_err(EngineInfoError::Eyre)
}

fn get_podman_info(
    ce: &Path,
    msg_info: &mut MessageInfo,
) -> Result<Option<(ContainerOs, Architecture)>, EngineInfoError> {
    engine_info(ce, &["info", "-f", "{{ .Version.OsArch }}"], "/", msg_info)
}

fn get_custom_info(
    ce: &Path,
    msg_info: &mut MessageInfo,
) -> Result<Option<(ContainerOs, Architecture)>, EngineInfoError> {
    engine_info(
        ce,
        &["version", "-f", "{{ .Client.Os }},,,{{ .Client.Arch }}"],
        ",,,",
        msg_info,
    )
}

pub fn get_container_engine() -> Result<PathBuf, which::Error> {
    if let Ok(ce) = env::var("CROSS_CONTAINER_ENGINE") {
        which::which(ce)
    } else {
        which::which(DOCKER).or_else(|_| which::which(PODMAN))
    }
}
