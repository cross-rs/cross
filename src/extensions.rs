use std::borrow::Cow;
use std::fmt;
use std::process::{Command, ExitStatus};

use crate::errors::*;

pub trait CommandExt {
    fn print_verbose(&self, verbose: bool);
    fn status_result(&self, status: ExitStatus) -> Result<()>;
    fn run(&mut self, verbose: bool) -> Result<()>;
    fn run_and_get_status(&mut self, verbose: bool) -> Result<ExitStatus>;
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String>;
    fn run_and_get_output(&mut self, verbose: bool) -> Result<std::process::Output>;
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
            eyre::bail!("`{:?}` failed with exit code: {:?}", self, status.code())
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
            .wrap_err_with(|| format!("couldn't execute `{:?}`", self))
    }

    /// Runs the command to completion and returns its stdout
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String> {
        let out = self.run_and_get_output(verbose)?;
        self.status_result(out.status)?;
        out.stdout()
    }

    /// Runs the command to completion and returns the status and its [output](std::process::Output).
    ///
    /// # Notes
    ///
    /// This command does not check the status.
    fn run_and_get_output(&mut self, verbose: bool) -> Result<std::process::Output> {
        self.print_verbose(verbose);
        self.output()
            .wrap_err_with(|| format!("couldn't execute `{:?}`", self))
            .map_err(Into::into)
    }
}

pub trait OutputExt {
    fn stdout(&self) -> Result<String>;
}

impl OutputExt for std::process::Output {
    fn stdout(&self) -> Result<String> {
        String::from_utf8(self.stdout.clone())
            .wrap_err_with(|| format!("`{:?}` output was not UTF-8", self))
    }
}

pub struct SafeCommand {
    program: String,
    args: Vec<String>,
}

impl SafeCommand {
    pub fn new<S: ToString>(program: S) -> Self {
        let program = program.to_string();
        SafeCommand {
            program,
            args: Vec::new(),
        }
    }

    pub fn arg<S>(&mut self, arg: &S) -> &mut Self
    where
        S: ToString,
    {
        self.args.push(arg.to_string());
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        for arg in args {
            self.arg(&arg);
        }
        self
    }
}

impl fmt::Debug for SafeCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&shell_escape::escape(Cow::from(&self.program)))?;
        for arg in &self.args {
            f.write_str(" ")?;
            f.write_str(&shell_escape::escape(Cow::from(arg)))?;
        }
        Ok(())
    }
}

impl From<SafeCommand> for Command {
    fn from(s: SafeCommand) -> Self {
        let mut cmd = Command::new(&s.program);
        cmd.args(&s.args);
        cmd
    }
}
