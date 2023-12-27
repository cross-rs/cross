use std::fs;

use super::containers::*;
use super::images::*;
use clap::Args;
use cross::shell::MessageInfo;

#[derive(Args, Debug)]
pub struct Clean {
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
    pub fn run(
        &self,
        engine: cross::docker::Engine,
        msg_info: &mut MessageInfo,
    ) -> cross::Result<()> {
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
            force: self.force,
            execute: self.execute,
            engine: None,
        };
        remove_containers.run(engine.clone(), msg_info)?;

        let remove_images = RemoveImages {
            targets: vec![],
            force: self.force,
            local: self.local,
            execute: self.execute,
            engine: None,
        };
        remove_images.run(engine.clone(), msg_info)?;

        let remove_volumes = RemoveAllVolumes {
            force: self.force,
            execute: self.execute,
            engine: None,
        };
        remove_volumes.run(engine.clone(), msg_info)?;

        let prune_volumes = PruneVolumes {
            execute: self.execute,
            engine: None,
        };
        prune_volumes.run(engine, msg_info)?;

        Ok(())
    }

    pub fn engine(&self) -> Option<&str> {
        self.engine.as_deref()
    }
}
