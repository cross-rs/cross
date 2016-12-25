use std::process::{Command, ExitStatus};

use errors::*;

pub trait CommandExt {
    fn run(&mut self) -> Result<()>;
    fn run_and_get_exit_status(&mut self) -> Result<ExitStatus>;
    fn run_and_get_stdout(&mut self) -> Result<String>;
}

impl CommandExt for Command {
    /// Runs the command to completion
    fn run(&mut self) -> Result<()> {
        let status = self.run_and_get_exit_status()?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("`{:?}` failed with exit code: {:?}",
                        self,
                        status.code()))?
        }
    }

    /// Runs the command to completion
    fn run_and_get_exit_status(&mut self) -> Result<ExitStatus> {
        self.status()
            .chain_err(|| format!("couldn't execute `{:?}`", self))
    }

    /// Runs the command to completion and returns its stdout
    fn run_and_get_stdout(&mut self) -> Result<String> {
        let out = self.output()
            .chain_err(|| format!("couldn't execute `{:?}`", self))?;

        if out.status.success() {
            Ok(String::from_utf8(out.stdout).chain_err(|| {
                    format!("`{:?}` output was not UTF-8",
                            self)
                })?)
        } else {
            Err(format!("`{:?}` failed with exit code: {:?}",
                        self,
                        out.status.code()))?
        }

    }
}
