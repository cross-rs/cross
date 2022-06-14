use std::borrow::Cow;
use std::fmt;
use std::process::{Command, ExitStatus};

use crate::errors::*;

pub trait CommandExt {
    fn print_verbose(&self, verbose: bool);
    fn status_result(&self, status: ExitStatus) -> Result<(), CommandError>;
    fn run(&mut self, verbose: bool) -> Result<(), CommandError>;
    fn run_and_get_status(&mut self, verbose: bool) -> Result<ExitStatus, CommandError>;
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String, CommandError>;
    fn run_and_get_output(&mut self, verbose: bool) -> Result<std::process::Output, CommandError>;
}

impl CommandExt for Command {
    fn print_verbose(&self, verbose: bool) {
        if verbose {
            println!("+ {:?}", self);
        }
    }

    fn status_result(&self, status: ExitStatus) -> Result<(), CommandError> {
        if status.success() {
            Ok(())
        } else {
            Err(CommandError::NonZeroExitCode(status, format!("{self:?}")))
        }
    }

    /// Runs the command to completion
    fn run(&mut self, verbose: bool) -> Result<(), CommandError> {
        let status = self.run_and_get_status(verbose)?;
        self.status_result(status)
    }

    /// Runs the command to completion
    fn run_and_get_status(&mut self, verbose: bool) -> Result<ExitStatus, CommandError> {
        self.print_verbose(verbose);
        self.status()
            .map_err(|e| CommandError::CouldNotExecute(Box::new(e), format!("{self:?}")))
    }

    /// Runs the command to completion and returns its stdout
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String, CommandError> {
        let out = self.run_and_get_output(verbose)?;
        self.status_result(out.status)?;
        out.stdout()
    }

    /// Runs the command to completion and returns the status and its [output](std::process::Output).
    ///
    /// # Notes
    ///
    /// This command does not check the status.
    fn run_and_get_output(&mut self, verbose: bool) -> Result<std::process::Output, CommandError> {
        self.print_verbose(verbose);
        self.output()
            .map_err(|e| CommandError::CouldNotExecute(Box::new(e), format!("{self:?}")))
    }
}

pub trait OutputExt {
    fn stdout(&self) -> Result<String, CommandError>;
}

impl OutputExt for std::process::Output {
    fn stdout(&self) -> Result<String, CommandError> {
        String::from_utf8(self.stdout.clone()).map_err(|e| CommandError::Utf8Error(e, self.clone()))
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
