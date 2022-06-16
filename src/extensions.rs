use std::borrow::Cow;
use std::fmt;
use std::process::{Command, ExitStatus, Output};

use crate::errors::*;

pub const STRIPPED_BINS: &[&str] = &[crate::docker::DOCKER, crate::docker::PODMAN, "cargo"];

pub trait CommandExt {
    fn print_verbose(&self, verbose: bool);
    fn status_result(
        &self,
        verbose: bool,
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
    fn command_pretty(&self, verbose: bool, strip: impl for<'a> Fn(&'a str) -> bool) -> String;
}

impl CommandExt for Command {
    fn command_pretty(&self, verbose: bool, strip: impl for<'a> Fn(&'a str) -> bool) -> String {
        // a dummy implementor of display to avoid using unwraps
        struct C<'c, F>(&'c Command, bool, F);
        impl<F> std::fmt::Display for C<'_, F>
        where
            F: for<'a> Fn(&'a str) -> bool,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let cmd = self.0;
                write!(
                    f,
                    "{}",
                    // if verbose, never strip, if not, let user choose
                    crate::file::pretty_path(cmd.get_program(), move |c| if self.1 {
                        false
                    } else {
                        self.2(c)
                    })
                )?;
                let args = cmd.get_args();
                if args.len() > 1 {
                    write!(
                        f,
                        " {}",
                        shell_words::join(args.map(|o| o.to_string_lossy()))
                    )?;
                }
                Ok(())
            }
        }
        format!("{}", C(self, verbose, strip))
    }

    fn print_verbose(&self, verbose: bool) {
        // TODO: introduce verbosity levels, v = 1, strip cmd, v > 1, don't strip cmd
        if verbose {
            if let Some(cwd) = self.get_current_dir() {
                println!("+ {:?} {}", cwd, self.command_pretty(true, |_| false));
            } else {
                println!("+ {}", self.command_pretty(true, |_| false));
            }
        }
    }

    fn status_result(
        &self,
        verbose: bool,
        status: ExitStatus,
        output: Option<&Output>,
    ) -> Result<(), CommandError> {
        if status.success() {
            Ok(())
        } else {
            Err(CommandError::NonZeroExitCode {
                status,
                command: self
                    .command_pretty(verbose, |ref cmd| STRIPPED_BINS.iter().any(|f| f == cmd)),
                stderr: output.map(|out| out.stderr.clone()).unwrap_or_default(),
                stdout: output.map(|out| out.stdout.clone()).unwrap_or_default(),
            })
        }
    }

    /// Runs the command to completion
    fn run(&mut self, verbose: bool, silence_stdout: bool) -> Result<(), CommandError> {
        let status = self.run_and_get_status(verbose, silence_stdout)?;
        self.status_result(verbose, status, None)
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
                command: self
                    .command_pretty(verbose, |ref cmd| STRIPPED_BINS.iter().any(|f| f == cmd)),
            })
            .map_err(Into::into)
    }

    /// Runs the command to completion and returns its stdout
    fn run_and_get_stdout(&mut self, verbose: bool) -> Result<String> {
        let out = self.run_and_get_output(verbose)?;
        self.status_result(verbose, out.status, Some(&out))
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
                command: self
                    .command_pretty(verbose, |ref cmd| STRIPPED_BINS.iter().any(|f| f == cmd)),
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

pub(crate) fn env_program(envvar: &str, program: &str) -> String {
    std::env::var(envvar)
        .ok()
        .unwrap_or_else(|| program.to_string())
}
