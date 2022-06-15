use std::fs;

use super::images::RemoveImages;
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
            false => println!("fs::remove_dir_all({})", tempdir.display()),
        }

        let remove_images = RemoveImages {
            targets: vec![],
            verbose: self.verbose,
            force: self.force,
            local: self.local,
            execute: self.execute,
            engine: None,
        };
        remove_images.run(engine)?;

        Ok(())
    }
}
