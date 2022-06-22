use std::fs;

use super::containers::*;
use super::images::*;
use clap::Args;

#[derive(Args, Debug)]
pub struct Clean {
    /// Provide verbose diagnostic output.
    #[clap(short, long)]
    pub verbose: bool,
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
        let tempdir = cross::temp::dir()?;
        match self.execute {
            true => {
                if tempdir.exists() {
                    fs::remove_dir_all(tempdir)?;
                }
            }
            false => println!(
                "fs::remove_dir_all({})",
                cross::pretty_path(&tempdir, |_| false)
            ),
        }

        // containers -> images -> volumes -> prune to ensure no conflicts.
        let remove_containers = RemoveAllContainers {
            verbose: self.verbose,
            force: self.force,
            execute: self.execute,
            engine: None,
        };
        remove_containers.run(engine.clone())?;

        let remove_images = RemoveImages {
            targets: vec![],
            verbose: self.verbose,
            force: self.force,
            local: self.local,
            execute: self.execute,
            engine: None,
        };
        remove_images.run(engine.clone())?;

        let remove_volumes = RemoveAllVolumes {
            verbose: self.verbose,
            force: self.force,
            execute: self.execute,
            engine: None,
        };
        remove_volumes.run(engine.clone())?;

        let prune_volumes = PruneVolumes {
            verbose: self.verbose,
            execute: self.execute,
            engine: None,
        };
        prune_volumes.run(engine)?;

        Ok(())
    }
}
