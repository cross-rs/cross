use std::borrow::Cow;
use std::fmt;
use std::process::{Command, ExitStatus, Output};

use crate::errors::*;
use crate::shell::MessageInfo;

pub const STRIPPED_BINS: &[&str] = &[crate::docker::DOCKER, crate::docker::PODMAN, "cargo"];

pub trait CommandExt {
    fn fmt_message(&self, msg_info: &mut MessageInfo) -> String;

    #[track_caller]
    fn print(&self, msg_info: &mut MessageInfo) -> Result<()> {
        let msg = self.fmt_message(msg_info);
        msg_info.print(&msg)
    }

    #[track_caller]
    fn info(&self, msg_info: &mut MessageInfo) -> Result<()> {
        let msg = self.fmt_message(msg_info);
        msg_info.info(&msg)
    }

    #[track_caller]
    fn debug(&self, msg_info: &mut MessageInfo) -> Result<()> {
        let msg = self.fmt_message(msg_info);
        msg_info.debug(&msg)
    }

    fn status_result(
        &self,
        msg_info: &mut MessageInfo,
        status: ExitStatus,
        output: Option<&Output>,
    ) -> Result<(), CommandError>;
    #[track_caller]
    fn run(&mut self, msg_info: &mut MessageInfo, silence_stdout: bool) -> Result<()>;
    #[track_caller]
    fn run_and_get_status(
        &mut self,
        msg_info: &mut MessageInfo,
        silence_stdout: bool,
    ) -> Result<ExitStatus>;
    #[track_caller]
    fn run_and_get_stdout(&mut self, msg_info: &mut MessageInfo) -> Result<String>;
    #[track_caller]
    fn run_and_get_output(&mut self, msg_info: &mut MessageInfo) -> Result<std::process::Output>;
    fn command_pretty(
        &self,
        msg_info: &mut MessageInfo,
        strip: impl for<'a> Fn(&'a str) -> bool,
    ) -> String;
}

impl CommandExt for Command {
    fn command_pretty(
        &self,
        msg_info: &mut MessageInfo,
        strip: impl for<'a> Fn(&'a str) -> bool,
    ) -> String {
        // a dummy implementor of display to avoid using unwraps
        struct C<'c, 'd, F>(&'c Command, &'d mut MessageInfo, F);
        impl<'e, 'f, F> std::fmt::Display for C<'e, 'f, F>
        where
            F: for<'a> Fn(&'a str) -> bool,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let cmd = self.0;
                write!(
                    f,
                    "{}",
                    // if verbose, never strip, if not, let user choose
                    crate::file::pretty_path(cmd.get_program(), move |c| if self.1.is_verbose() {
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
        format!("{}", C(self, msg_info, strip))
    }

    #[track_caller]
    fn fmt_message(&self, mut msg_info: &mut MessageInfo) -> String {
        use std::fmt::Write;
        let msg_info = &mut msg_info;
        let mut string = String::new();
        if let Some(caller) = msg_info.caller() {
            write!(string, "[{}] ->\n+ ", caller).unwrap();
        } else {
            write!(string, "+ ").unwrap();
        };
        if let Some(cwd) = self.get_current_dir() {
            write!(
                string,
                "{:?} {}",
                cwd,
                msg_info.as_verbose(|info| self.command_pretty(info, |_| false))
            )
            .unwrap();
        } else {
            write!(
                string,
                "{}",
                msg_info.as_verbose(|info| self.command_pretty(info, |_| false))
            )
            .unwrap();
        }
        string
    }

    #[track_caller]
    fn status_result(
        &self,
        msg_info: &mut MessageInfo,
        status: ExitStatus,
        output: Option<&Output>,
    ) -> Result<(), CommandError> {
        if status.success() {
            Ok(())
        } else {
            Err(CommandError::NonZeroExitCode {
                status,
                command: self
                    .command_pretty(msg_info, |cmd| STRIPPED_BINS.iter().any(|f| f == &cmd)),
                stderr: output.map(|out| out.stderr.clone()).unwrap_or_default(),
                stdout: output.map(|out| out.stdout.clone()).unwrap_or_default(),
            })
        }
    }

    /// Runs the command to completion
    #[track_caller]
    fn run(&mut self, msg_info: &mut MessageInfo, silence_stdout: bool) -> Result<()> {
        let status = self.run_and_get_status(msg_info, silence_stdout)?;
        #[warn(clippy::nursery)]
        Ok(self.status_result(msg_info, status, None)?)
    }

    /// Runs the command to completion
    #[track_caller]
    fn run_and_get_status(
        &mut self,
        msg_info: &mut MessageInfo,
        silence_stdout: bool,
    ) -> Result<ExitStatus> {
        self.debug(msg_info)?;
        if silence_stdout && !msg_info.is_verbose() {
            self.stdout(std::process::Stdio::null());
        }
        Ok(self.status().map_err(|e| CommandError::CouldNotExecute {
            source: Box::new(e),
            command: self.command_pretty(msg_info, |cmd| STRIPPED_BINS.iter().any(|f| f == &cmd)),
        })?)
    }

    /// Runs the command to completion and returns its stdout
    #[track_caller]
    fn run_and_get_stdout(&mut self, msg_info: &mut MessageInfo) -> Result<String> {
        let out = self.run_and_get_output(msg_info)?;
        self.status_result(msg_info, out.status, Some(&out))
            .map_err(CommandError::to_section_report)?;
        Ok(out.stdout()?)
    }

    /// Runs the command to completion and returns the status and its [output](std::process::Output).
    ///
    /// # Notes
    ///
    /// This command does not check the status.
    #[track_caller]
    fn run_and_get_output(&mut self, msg_info: &mut MessageInfo) -> Result<std::process::Output> {
        self.debug(msg_info)?;
        self.output().map_err(|e| {
            CommandError::CouldNotExecute {
                source: Box::new(e),
                command: self
                    .command_pretty(msg_info, |cmd| STRIPPED_BINS.iter().any(|f| f == &cmd)),
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
        .unwrap_or_else(|| program.to_owned())
}
