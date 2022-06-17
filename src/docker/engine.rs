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

    let mut cmd = Command::new(ce);
    cmd.args(&["version", "-f", "{{ .Server.Os }},,,{{ .Server.Arch }}"]);

    let out = cmd.run_and_get_output(msg_info)?;

    let stdout = out.stdout()?.to_lowercase();

    let osarch = stdout
        .trim()
        .split_once(",,,")
        .map(|(os, arch)| -> Result<_> { Ok((ContainerOs::new(os)?, Architecture::new(arch)?)) })
        .transpose();

    let osarch = match (kind, osarch) {
        (_, Ok(Some(osarch))) => Some(osarch),
        (EngineType::PodmanRemote | EngineType::Podman, Ok(None)) => get_podman_info(ce, msg_info)?,
        (_, Err(e)) => {
            return Err(e.wrap_err(format!(
                "command `{}` returned unexpected data",
                cmd.command_pretty(msg_info, |_| false)
            )));
        }
        (EngineType::Docker | EngineType::Other, Ok(None)) => None,
    };

    let osarch = if osarch.is_some() {
        osarch
    } else if !out.status.success() {
        get_custom_info(ce, msg_info).with_error(|| {
            cmd.status_result(msg_info, out.status, Some(&out))
                .expect_err("status_result should error")
        })?
    } else {
        get_custom_info(ce, msg_info)?
    };

    let (os, arch) = osarch.map_or(<_>::default(), |(os, arch)| (Some(os), Some(arch)));
    Ok((kind, arch, os))
}

fn get_podman_info(
    ce: &Path,
    msg_info: &mut MessageInfo,
) -> Result<Option<(ContainerOs, Architecture)>> {
    let mut cmd = Command::new(ce);
    cmd.args(&["info", "-f", "{{ .Version.OsArch }}"]);
    cmd.run_and_get_stdout(msg_info)
        .map(|s| {
            s.to_lowercase()
                .trim()
                .split_once('/')
                .map(|(os, arch)| -> Result<_> {
                    Ok((ContainerOs::new(os)?, Architecture::new(arch)?))
                })
        })
        .wrap_err("could not determine os and architecture of vm")?
        .transpose()
}

fn get_custom_info(
    ce: &Path,
    msg_info: &mut MessageInfo,
) -> Result<Option<(ContainerOs, Architecture)>> {
    let mut cmd = Command::new(ce);
    cmd.args(&["info", "-f", "{{ .Client.Os }},,,{{ .Client.Arch }}"]);
    cmd.run_and_get_stdout(msg_info)
        .map(|s| {
            s.to_lowercase()
                .trim()
                .split_once(",,,")
                .map(|(os, arch)| -> Result<_> {
                    Ok((ContainerOs::new(os)?, Architecture::new(arch)?))
                })
        })
        .unwrap_or_default()
        .transpose()
}

pub fn get_container_engine() -> Result<PathBuf, which::Error> {
    if let Ok(ce) = env::var("CROSS_CONTAINER_ENGINE") {
        which::which(ce)
    } else {
        which::which(DOCKER).or_else(|_| which::which(PODMAN))
    }
}
