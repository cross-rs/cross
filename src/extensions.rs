use std::process::{Command, ExitStatus};

use crate::errors::*;

pub trait CommandExt {
    fn print_verbose(&self, verbose: bool);
    fn status_result(&self, status: ExitStatus) -> Result<()>;
    fn run(&mut self, verbose: bool) -> Result<()>;
    fn run_and_get_status(&mut self, verbose: bool) -> Result<ExitStatus>;
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String>;
}

impl CommandExt for Command {
    fn print_verbose(&self, verbose: bool) {
        if verbose {
            println!("+ {:?}", self);
        }
    }

    fn status_result(&self, status: ExitStatus) -> Result<()> {
        if status.success() {
            Ok(())
        } else {
            Err(format!("`{:?}` failed with exit code: {:?}", self, status.code()).into())
        }
    }

    /// Runs the command to completion
    fn run(&mut self, verbose: bool) -> Result<()> {
        let status = self.run_and_get_status(verbose)?;
        self.status_result(status)
    }

    /// Runs the command to completion
    fn run_and_get_status(&mut self, verbose: bool) -> Result<ExitStatus> {
        self.print_verbose(verbose);
        self.status()
            .chain_err(|| format!("couldn't execute `{:?}`", self))
    }

    /// Runs the command to completion and returns its stdout
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String> {
        self.print_verbose(verbose);
        let out = self.output()
            .chain_err(|| format!("couldn't execute `{:?}`", self))?;

        self.status_result(out.status)?;

        Ok(String::from_utf8(out.stdout)
            .chain_err(|| format!("`{:?}` output was not UTF-8", self))?)
    }
}
