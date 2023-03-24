// This file was adapted from:
//   https://github.com/rust-lang/cargo/blob/ca4edabb28fc96fdf2a1d56fe3851831ac166f8a/src/cargo/core/shell.rs

use std::env;
use std::fmt;
use std::io::{self, Write};
use std::str::FromStr;

use crate::config::bool_from_envvar;
use crate::errors::Result;
use is_terminal::IsTerminal;
use owo_colors::{self, OwoColorize};

// get the prefix for stderr messages
macro_rules! cross_prefix {
    ($s:literal) => {
        concat!("[cross]", " ", $s)
    };
}

// generate the color style
macro_rules! write_style {
    ($stream:ident, $msg_info:expr, $message:expr $(, $style:ident)* $(,)?) => {{
        match $msg_info.color_choice {
            ColorChoice::Always => write!($stream, "{}", $message $(.$style())*),
            ColorChoice::Never => write!($stream, "{}", $message),
            ColorChoice::Auto => write!(
                $stream,
                "{}",
                $message $(.if_supports_color($stream.owo(), |text| text.$style()))*
            ),
        }?;
    }};
}

// low-level interface for printing colorized messages
macro_rules! message {
    // write a status message, which has the following format:
    //  "{status}: {message}"
    // both status and ':' are bold.
    (@status $stream:ident, $status:expr, $message:expr, $color:ident, $msg_info:expr $(,)?) => {{
        write_style!($stream, $msg_info, $status, bold, $color);
        write_style!($stream, $msg_info, ":", bold);
        if let Some(caller) = $msg_info.caller() {
            write!($stream, " [{}]", caller)?;
        }
        match $message {
            Some(message) => writeln!($stream, " {}", message, )?,
            None => write!($stream, " ")?,
        }

        Ok(())
    }};

    (@status @name $name:ident, $status:expr, $message:expr, $color:ident, $msg_info:expr $(,)?) => {{
        let mut stream = io::$name();
        message!(@status stream, $status, $message, $color, $msg_info)
    }};
}

// high-level interface to message
macro_rules! status {
    (@stderr $status:expr, $message:expr, $color:ident, $msg_info:expr $(,)?) => {{
        message!(@status @name stderr, $status, $message, $color, $msg_info)
    }};

    (@stdout $status:expr, $message:expr, $color:ident, $msg_info:expr  $(,)?) => {{
        message!(@status @name stdout, $status, $message, $color, $msg_info)
    }};
}

/// the requested verbosity of output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    Quiet,
    #[default]
    Normal,
    Verbose(u8),
}

impl Verbosity {
    pub fn verbose(self) -> bool {
        match self {
            Self::Verbose(..) => true,
            Self::Normal | Self::Quiet => false,
        }
    }

    #[must_use]
    pub fn level(&self) -> u8 {
        match &self {
            Verbosity::Verbose(v) => *v,
            _ => 0,
        }
    }

    fn create(color_choice: ColorChoice, verbose: impl Into<u8>, quiet: bool) -> Option<Self> {
        match (verbose.into(), quiet) {
            (1.., true) => {
                MessageInfo::from(color_choice).fatal("cannot set both --verbose and --quiet", 101)
            }
            (v @ 1.., false) => Some(Verbosity::Verbose(v)),
            (0, true) => Some(Verbosity::Quiet),
            (0, false) => None,
        }
    }
}

/// Whether messages should use color output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    /// force color output
    Always,
    /// force disable color output
    Never,
    /// intelligently guess whether to use color output
    Auto,
}

impl FromStr for ColorChoice {
    type Err = eyre::ErrReport;

    fn from_str(s: &str) -> Result<ColorChoice> {
        match s {
            "always" => Ok(ColorChoice::Always),
            "never" => Ok(ColorChoice::Never),
            "auto" => Ok(ColorChoice::Auto),
            arg => eyre::bail!(
                "argument for --color must be auto, always, or never, but found `{arg}`"
            ),
        }
    }
}

// Should simplify the APIs a lot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageInfo {
    pub color_choice: ColorChoice,
    pub verbosity: Verbosity,
    pub stdout_needs_erase: bool,
    pub stderr_needs_erase: bool,
    pub cross_debug: bool,
    pub has_warned: bool,
}

impl MessageInfo {
    pub fn new(color_choice: ColorChoice, verbosity: Verbosity) -> MessageInfo {
        MessageInfo {
            color_choice,
            verbosity,
            stdout_needs_erase: false,
            stderr_needs_erase: false,
            cross_debug: std::env::var("CROSS_DEBUG")
                .as_deref()
                .map(bool_from_envvar)
                .unwrap_or_default(),
            has_warned: false,
        }
    }

    pub fn create(verbose: impl Into<u8>, quiet: bool, color: Option<&str>) -> Result<MessageInfo> {
        let color_choice = get_color_choice(color)?;
        let verbosity = get_verbosity(color_choice, verbose, quiet)?;

        Ok(Self::new(color_choice, verbosity))
    }

    #[track_caller]
    pub fn caller(&mut self) -> Option<impl fmt::Display> {
        if self.cross_debug {
            let loc = std::panic::Location::caller();
            Some(format!("{}:{}", loc.file(), loc.line()))
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_verbose(&self) -> bool {
        self.verbosity.verbose()
    }

    fn as_verbosity<T, C: Fn(&mut MessageInfo) -> T>(&mut self, call: C, new: Verbosity) -> T {
        let old = self.verbosity;
        self.verbosity = new;
        let result = call(self);
        self.verbosity = old;

        result
    }

    pub fn as_quiet<T, C: Fn(&mut MessageInfo) -> T>(&mut self, call: C) -> T {
        self.as_verbosity(call, Verbosity::Quiet)
    }

    pub fn as_normal<T, C: Fn(&mut MessageInfo) -> T>(&mut self, call: C) -> T {
        self.as_verbosity(call, Verbosity::Normal)
    }

    pub fn as_verbose<T, C: Fn(&mut MessageInfo) -> T>(&mut self, call: C) -> T {
        self.as_verbosity(call, Verbosity::Verbose(2))
    }

    fn erase_line<S: Stream + Write>(&mut self, stream: &mut S) -> Result<()> {
        // this is the Erase in Line sequence
        stream.write_all(b"\x1B[K").map_err(Into::into)
    }

    fn stdout_check_erase(&mut self) -> Result<()> {
        if self.stdout_needs_erase {
            self.erase_line(&mut io::stdout())?;
            self.stdout_needs_erase = false;
        }
        Ok(())
    }

    fn stderr_check_erase(&mut self) -> Result<()> {
        if self.stderr_needs_erase {
            self.erase_line(&mut io::stderr())?;
            self.stderr_needs_erase = false;
        }
        Ok(())
    }

    /// prints a red 'error' message and terminates.
    #[track_caller]
    pub fn fatal<T: fmt::Display>(&mut self, message: T, code: i32) -> ! {
        self.error(message)
            .expect("could not display fatal message");
        std::process::exit(code);
    }

    /// prints a red 'error' message.
    #[track_caller]
    pub fn error<T: fmt::Display>(&mut self, message: T) -> Result<()> {
        self.has_warned = true;
        self.stderr_check_erase()?;
        status!(@stderr cross_prefix!("error"), Some(&message), red, self)
    }

    /// prints an amber 'warning' message.
    #[track_caller]
    pub fn warn<T: fmt::Display>(&mut self, message: T) -> Result<()> {
        self.has_warned = true;
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => status!(@stderr
                cross_prefix!("warning"),
                Some(&message),
                yellow,
                self,
            ),
        }
    }

    /// prints a cyan 'note' message.
    #[track_caller]
    pub fn note<T: fmt::Display>(&mut self, message: T) -> Result<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => status!(@stderr cross_prefix!("note"), Some(&message), cyan, self),
        }
    }

    pub fn status<T: fmt::Display>(&mut self, message: T) -> Result<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => {
                eprintln!("{}", message);
                Ok(())
            }
        }
    }

    /// prints a high-priority message to stdout.
    #[track_caller]
    pub fn print<T: fmt::Display>(&mut self, message: T) -> Result<()> {
        self.stdout_check_erase()?;
        println!("{}", message);
        Ok(())
    }

    /// prints a normal message to stdout.
    #[track_caller]
    pub fn info<T: fmt::Display>(&mut self, message: T) -> Result<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => {
                println!("{}", message);
                Ok(())
            }
        }
    }

    /// prints a debugging message to stdout.
    #[track_caller]
    pub fn debug<T: fmt::Display>(&mut self, message: T) -> Result<()> {
        match self.verbosity {
            Verbosity::Quiet | Verbosity::Normal => Ok(()),
            _ => {
                println!("{}", message);
                Ok(())
            }
        }
    }

    pub fn fatal_usage<T: fmt::Display>(
        &mut self,
        arg: T,
        provided: Option<&str>,
        possible: Option<&[&str]>,
        code: i32,
    ) -> ! {
        self.error_usage(arg, provided, possible)
            .expect("could not display usage message");
        std::process::exit(code);
    }

    fn error_usage<T: fmt::Display>(
        &mut self,
        arg: T,
        provided: Option<&str>,
        possible: Option<&[&str]>,
    ) -> Result<()> {
        let mut stream = io::stderr();
        write_style!(stream, self, cross_prefix!("error"), bold, red);
        write_style!(stream, self, ":", bold);
        match provided {
            Some(value) => {
                write_style!(
                    stream,
                    self,
                    format_args!(" \"{value}\" isn't a valid value for '")
                );
                write_style!(stream, self, arg, yellow);
                write_style!(stream, self, "'\n");
            }
            None => {
                write_style!(stream, self, " The argument '");
                write_style!(stream, self, arg, yellow);
                write_style!(stream, self, "' requires a value but none was supplied\n");
            }
        }
        match possible {
            Some(values) if !values.is_empty() => {
                let error_indent = cross_prefix!("error: ").len();
                write_style!(
                    stream,
                    self,
                    format_args!("{:error_indent$}[possible values: ", "")
                );
                let max_index = values.len() - 1;
                for (index, value) in values.iter().enumerate() {
                    write_style!(stream, self, value, green);
                    if index < max_index {
                        write_style!(stream, self, ", ");
                    }
                }
                write_style!(stream, self, "]\n");
            }
            _ => (),
        }
        write_style!(stream, self, "Usage:\n");
        write_style!(
            stream,
            self,
            "    cross [+toolchain] [OPTIONS] [SUBCOMMAND]\n"
        );
        write_style!(stream, self, "\n");
        write_style!(stream, self, "For more information try ");
        write_style!(stream, self, "--help", green);
        write_style!(stream, self, "\n");

        stream.flush()?;

        Ok(())
    }

    /// Returns true if we've previously warned or errored, and we're in CI or `CROSS_NO_WARNINGS` has been set.
    ///
    /// This is used so that unexpected warnings and errors cause ci to fail.
    pub fn should_fail(&self) -> bool {
        // FIXME: store env var
        env::var("CROSS_NO_WARNINGS").map_or_else(|_| is_ci::cached(), |env| bool_from_envvar(&env))
            && self.has_warned
    }
}

impl Default for MessageInfo {
    fn default() -> MessageInfo {
        MessageInfo::new(ColorChoice::Auto, Verbosity::Normal)
    }
}

impl From<ColorChoice> for MessageInfo {
    fn from(color_choice: ColorChoice) -> MessageInfo {
        MessageInfo::new(color_choice, Verbosity::Normal)
    }
}

impl From<Verbosity> for MessageInfo {
    fn from(verbosity: Verbosity) -> MessageInfo {
        MessageInfo::new(ColorChoice::Auto, verbosity)
    }
}

impl From<(ColorChoice, Verbosity)> for MessageInfo {
    fn from(info: (ColorChoice, Verbosity)) -> MessageInfo {
        MessageInfo::new(info.0, info.1)
    }
}

// cargo only accepts literal booleans for some values.
pub fn cargo_envvar_bool(var: &str) -> Result<bool> {
    match env::var(var).ok() {
        Some(value) => value.parse::<bool>().map_err(|_ignore| {
            eyre::eyre!("environment variable for `{var}` was not `true` or `false`.")
        }),
        None => Ok(false),
    }
}

pub fn invalid_color(provided: Option<&str>) -> ! {
    let possible = ["auto", "always", "never"];
    MessageInfo::default().fatal_usage("--color <WHEN>", provided, Some(&possible), 1);
}

fn get_color_choice(color: Option<&str>) -> Result<ColorChoice> {
    Ok(match color {
        Some(arg) => arg.parse().unwrap_or_else(|_| invalid_color(color)),
        None => match env::var("CARGO_TERM_COLOR").ok().as_deref() {
            Some(arg) => arg.parse().unwrap_or_else(|_| invalid_color(color)),
            None => ColorChoice::Auto,
        },
    })
}

fn get_verbosity(
    color_choice: ColorChoice,
    verbose: impl Into<u8>,
    quiet: bool,
) -> Result<Verbosity> {
    // cargo always checks the value of these variables.
    let env_verbose = cargo_envvar_bool("CARGO_TERM_VERBOSE")?;
    let env_quiet = cargo_envvar_bool("CARGO_TERM_QUIET")?;
    Ok(match Verbosity::create(color_choice, verbose, quiet) {
        Some(v) => v,
        None => Verbosity::create(color_choice, env_verbose, env_quiet).unwrap_or_default(),
    })
}

pub trait Stream {
    type TTY: IsTerminal;
    const OWO: owo_colors::Stream;

    #[must_use]
    fn is_atty() -> bool;

    fn owo(&self) -> owo_colors::Stream {
        Self::OWO
    }
}

impl Stream for io::Stdin {
    type TTY = io::Stdin;
    const OWO: owo_colors::Stream = owo_colors::Stream::Stdin;

    fn is_atty() -> bool {
        io::stdin().is_terminal()
    }
}

impl Stream for io::Stdout {
    type TTY = io::Stdout;
    const OWO: owo_colors::Stream = owo_colors::Stream::Stdout;

    fn is_atty() -> bool {
        io::stdout().is_terminal()
    }
}

impl Stream for io::Stderr {
    type TTY = io::Stderr;
    const OWO: owo_colors::Stream = owo_colors::Stream::Stderr;

    fn is_atty() -> bool {
        io::stderr().is_terminal()
    }
}

pub fn default_ident() -> usize {
    cross_prefix!("").len()
}

#[must_use]
pub fn indent(message: &str, spaces: usize) -> String {
    use std::fmt::Write as _;
    message.lines().fold(String::new(), |mut string, line| {
        let _ = write!(string, "{:spaces$}{line}", "");
        string
    })
}
