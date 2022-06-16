use std::borrow::Cow;
use std::fmt;
use std::process::{Command, ExitStatus, Output};

use crate::errors::*;

pub trait CommandExt {
    fn print_verbose(&self, verbose: bool);
    fn status_result(
        &self,
        status: ExitStatus,
        output: Option<&Output>,
    ) -> Result<(), CommandError>;
    fn run(&mut self, verbose: bool, silence_stdout: bool) -> Result<(), CommandError>;
    fn run_and_get_status(
        &mut self,
        verbose: bool,
        silence_stdout: bool,
    ) -> Result<ExitStatus, CommandError>;
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String>;
    fn run_and_get_output(&mut self, verbose: bool) -> Result<std::process::Output>;
    fn command_pretty(&self) -> String;
}

impl CommandExt for Command {
    fn command_pretty(&self) -> String {
        // a dummy implementor of display to avoid using unwraps
        struct C<'c>(&'c Command);
        impl std::fmt::Display for C<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let cmd = self.0;
                write!(f, "{}", cmd.get_program().to_string_lossy())?;
                let args = cmd.get_args();
                if args.len() > 1 {
                    write!(f, " ")?;
                    write!(
                        f,
                        "{}",
                        shell_words::join(args.map(|o| o.to_string_lossy()))
                    )?;
                }
                Ok(())
            }
        }
        format!("{}", C(self))
    }

    fn print_verbose(&self, verbose: bool) {
        if verbose {
            if let Some(cwd) = self.get_current_dir() {
                println!("+ {:?} {}", cwd, self.command_pretty());
            } else {
                println!("+ {}", self.command_pretty());
            }
        }
    }

    fn status_result(
        &self,
        status: ExitStatus,
        output: Option<&Output>,
    ) -> Result<(), CommandError> {
        if status.success() {
            Ok(())
        } else {
            Err(CommandError::NonZeroExitCode {
                status,
                command: self.command_pretty(),
                stderr: output.map(|out| out.stderr.clone()).unwrap_or_default(),
                stdout: output.map(|out| out.stdout.clone()).unwrap_or_default(),
            })
        }
    }

    /// Runs the command to completion
    fn run(&mut self, verbose: bool, silence_stdout: bool) -> Result<(), CommandError> {
        let status = self.run_and_get_status(verbose, silence_stdout)?;
        self.status_result(status, None)
    }

    /// Runs the command to completion
    fn run_and_get_status(
        &mut self,
        verbose: bool,
        silence_stdout: bool,
    ) -> Result<ExitStatus, CommandError> {
        self.print_verbose(verbose);
        if silence_stdout && !verbose {
            self.stdout(std::process::Stdio::null());
        }
        self.status()
            .map_err(|e| CommandError::CouldNotExecute {
                source: Box::new(e),
                command: self.command_pretty(),
            })
            .map_err(Into::into)
    }

    /// Runs the command to completion and returns its stdout
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String> {
        let out = self.run_and_get_output(verbose)?;
        self.status_result(out.status, Some(&out))
            .map_err(CommandError::to_section_report)?;
        out.stdout().map_err(Into::into)
    }

    /// Runs the command to completion and returns the status and its [output](std::process::Output).
    ///
    /// # Notes
    ///
    /// This command does not check the status.
    fn run_and_get_output(&mut self, verbose: bool) -> Result<std::process::Output> {
        self.print_verbose(verbose);
        self.output().map_err(|e| {
            CommandError::CouldNotExecute {
                source: Box::new(e),
                command: self.command_pretty(),
            }
            .to_section_report()
        })
    }
}

pub trait OutputExt {
    fn stdout(&self) -> Result<String, CommandError>;
    fn stderr(&self) -> Result<String, CommandError>;
}

impl OutputExt for std::process::Output {
    fn stdout(&self) -> Result<String, CommandError> {
        String::from_utf8(self.stdout.clone()).map_err(|e| CommandError::Utf8Error(e, self.clone()))
    }

    fn stderr(&self) -> Result<String, CommandError> {
        String::from_utf8(self.stderr.clone()).map_err(|e| CommandError::Utf8Error(e, self.clone()))
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
