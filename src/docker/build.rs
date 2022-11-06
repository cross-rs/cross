use std::env;
use std::process::Command;
use std::str::FromStr;

use super::engine::Engine;
use crate::errors::*;
use crate::shell::Verbosity;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Progress {
    Plain,
    Auto,
    Tty,
}

impl FromStr for Progress {
    type Err = eyre::ErrReport;

    fn from_str(progress: &str) -> Result<Self> {
        Ok(match progress {
            "plain" => Progress::Plain,
            "auto" => Progress::Auto,
            "tty" => Progress::Tty,
            s => eyre::bail!("unexpect progress type: expected plain, auto, or tty and got {s}"),
        })
    }
}

impl From<Progress> for &str {
    fn from(progress: Progress) -> Self {
        match progress {
            Progress::Plain => "plain",
            Progress::Auto => "auto",
            Progress::Tty => "tty",
        }
    }
}

pub trait BuildCommandExt {
    fn invoke_build_command(&mut self) -> &mut Self;
    fn progress(&mut self, progress: Option<Progress>) -> Result<&mut Self>;
    fn verbose(&mut self, verbosity: Verbosity) -> &mut Self;
    fn disable_scan_suggest(&mut self) -> &mut Self;
    fn cross_labels(&mut self, target: &str, platform: &str) -> &mut Self;
}

impl BuildCommandExt for Command {
    fn invoke_build_command(&mut self) -> &mut Self {
        match Engine::has_buildkit() {
            true => self.args(["buildx", "build"]),
            false => self.arg("build"),
        }
    }

    fn progress(&mut self, progress: Option<Progress>) -> Result<&mut Self> {
        let progress: Progress = match progress {
            None => env::var("CROSS_BUILD_PROGRESS")
                .as_deref()
                .unwrap_or("auto")
                .parse()?,
            Some(progress) => progress,
        };
        Ok(self.args(["--progress", progress.into()]))
    }

    fn verbose(&mut self, verbosity: Verbosity) -> &mut Self {
        match verbosity {
            Verbosity::Verbose(2..) => self.args(["--build-arg", "VERBOSE=1"]),
            _ => self,
        }
    }

    fn disable_scan_suggest(&mut self) -> &mut Self {
        self.env("DOCKER_SCAN_SUGGEST", "false")
    }

    fn cross_labels(&mut self, target: &str, platform: &str) -> &mut Self {
        self.args([
            "--label",
            &format!("{}.for-cross-target={target}", crate::CROSS_LABEL_DOMAIN,),
        ]);
        self.args([
            "--label",
            &format!("{}.runs-with={platform}", crate::CROSS_LABEL_DOMAIN,),
        ])
    }
}

pub trait BuildResultExt {
    fn engine_warning(self, engine: &Engine) -> Result<()>;
    fn buildkit_warning(self) -> Result<()>;
}

impl BuildResultExt for Result<()> {
    fn engine_warning(self, engine: &Engine) -> Result<()> {
        self.with_warning(|| {
            format!(
                "call to {} failed",
                engine
                    .path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map_or_else(|| "container engine", |s| s)
            )
        })
    }

    fn buildkit_warning(mut self) -> Result<()> {
        if Engine::has_buildkit() {
            self = self
                .suggestion("is `buildx` available for the container engine?")
                .with_note(|| {
                    format!(
                        "disable the `buildkit` dependency optionally with `{}=1`",
                        Engine::CROSS_CONTAINER_ENGINE_NO_BUILDKIT_ENV
                    )
                });
        }
        self
    }
}
