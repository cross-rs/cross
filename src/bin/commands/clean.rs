use std::fs;

use super::containers::*;
use super::images::*;
use clap::Args;
use cross::shell::MessageInfo;

#[derive(Args, Debug)]
pub struct Clean {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
    /// Do not print cross log messages.
    #[clap(short, long)]
    pub quiet: bool,
    /// Whether messages should use color output.
    #[clap(long)]
    pub color: Option<String>,
    /// Force removal of images.
    #[clap(short, long)]
    pub force: bool,
    /// Remove local (development) images.
    #[clap(short, long)]
    pub local: bool,
    /// Remove images. Default is a dry run.
    #[clap(short, long)]
    pub execute: bool,
    /// Container engine (such as docker or podman).
    #[clap(long)]
    pub engine: Option<String>,
}

impl Clean {
    pub fn run(self, engine: cross::docker::Engine) -> cross::Result<()> {
        let mut msg_info = MessageInfo::create(self.verbose, self.quiet, self.color.as_deref())?;
        let tempdir = cross::temp::dir()?;
        match self.execute {
            true => {
                if tempdir.exists() {
                    fs::remove_dir_all(tempdir)?;
                }
            }
            false => msg_info.print(format_args!(
                "fs::remove_dir_all({})",
                cross::pretty_path(&tempdir, |_| false)
            ))?,
        }

        // containers -> images -> volumes -> prune to ensure no conflicts.
        let remove_containers = RemoveAllContainers {
            verbose: self.verbose,
            quiet: self.quiet,
            color: self.color.clone(),
            force: self.force,
            execute: self.execute,
            engine: None,
        };
        remove_containers.run(engine.clone())?;

        let remove_images = RemoveImages {
            targets: vec![],
            verbose: self.verbose,
            quiet: self.quiet,
            color: self.color.clone(),
            force: self.force,
            local: self.local,
            execute: self.execute,
            engine: None,
        };
        remove_images.run(engine.clone())?;

        let remove_volumes = RemoveAllVolumes {
            verbose: self.verbose,
            quiet: self.quiet,
            color: self.color.clone(),
            force: self.force,
            execute: self.execute,
            engine: None,
        };
        remove_volumes.run(engine.clone())?;

        let prune_volumes = PruneVolumes {
            verbose: self.verbose,
            quiet: self.quiet,
            color: self.color.clone(),
            execute: self.execute,
            engine: None,
        };
        prune_volumes.run(engine)?;

        Ok(())
    }

    pub fn engine(&self) -> Option<&str> {
        self.engine.as_deref()
    }

    pub fn verbose(&self) -> bool {
        self.verbose
    }

    pub fn quiet(&self) -> bool {
        self.quiet
    }

    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }
}
