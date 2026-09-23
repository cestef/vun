//! Declarative command trees: argument parsing, generated help and usage errors as diagnostics.
//!
//! A program is a tree of [`Command`]s declared as data. Parsing walks the tree, so names,
//! parameters and help are declared once, and every front end (`overt`, `cargo xtask`) accepts
//! the same syntax: `<command>... [--flag] [--option value] <argument>...`, with `-h`/`--help`
//! everywhere, `help <command>...` in groups, and `-V`/`--version` on the root.

mod help;
mod parse;

use std::io::{self, Write};
use std::process::ExitCode;

use overt_diag::{Diagnostic, Msg, errors};

pub use help::help;
pub use parse::{Request, Selected, parse};

pub struct Program<Cx: 'static> {
    pub version: &'static str,
    pub root: Command<Cx>,
    /// Applies the options a command inherits to the context, before the command runs.
    pub configure: Configure<Cx>,
}

/// Reads inherited options into the context; an invalid value is a usage error.
pub type Configure<Cx> = fn(&mut Cx, &Args) -> Result<(), Box<Diagnostic>>;

/// A [`Configure`] for programs whose groups declare no options.
///
/// # Errors
/// Never.
pub fn unconfigured<Cx>(_: &mut Cx, _: &Args) -> Result<(), Box<Diagnostic>> {
    Ok(())
}

pub struct Command<Cx: 'static> {
    pub name: &'static str,
    pub about: &'static str,
    pub params: &'static [Param],
    pub action: Action<Cx>,
}

pub enum Action<Cx: 'static> {
    Run(Run<Cx>),
    Group(&'static [Command<Cx>]),
    /// Declared, so that help lists it, but not implemented yet.
    Planned,
}

/// A command's implementation. Output goes to `out`; diagnostics go through `Cx`.
pub type Run<Cx> = fn(&mut Cx, &Args, &mut dyn Write) -> io::Result<Outcome>;

impl<Cx> Command<Cx> {
    #[must_use]
    pub fn children(&self) -> &'static [Self] {
        match self.action {
            Action::Group(children) => children,
            Action::Run(_) | Action::Planned => &[],
        }
    }
}

pub struct Param {
    pub name: &'static str,
    pub short: Option<char>,
    pub about: &'static str,
    pub kind: ParamKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// A required argument, in declaration order: `<name>`.
    Positional,
    /// A switch: `--name`.
    Flag,
    /// A named value: `--name <value>` or `--name=value`.
    Option,
}

/// Options every command accepts.
pub const HELP: Param =
    Param { name: "help", short: Some('h'), about: "show this help", kind: ParamKind::Flag };
/// Options the root command accepts.
pub const VERSION: Param =
    Param { name: "version", short: Some('V'), about: "show the version", kind: ParamKind::Flag };
/// The group command that shows help for the commands after it.
pub const HELP_COMMAND: &str = "help";

/// The values given to a command's parameters.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    values: Vec<(&'static str, String)>,
    flags: Vec<&'static str>,
}

impl Args {
    /// The value of a positional or option parameter. Positional parameters are always present
    /// when a command runs.
    #[must_use]
    pub fn value(&self, param: &Param) -> Option<&str> {
        self.values.iter().find(|(name, _)| *name == param.name).map(|(_, v)| v.as_str())
    }

    #[must_use]
    pub fn flag(&self, param: &Param) -> bool {
        self.flags.contains(&param.name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Failure,
    /// Command-line misuse, reported as `EX_USAGE` from `sysexits.h`.
    Usage,
}

/// Exit status of command-line misuse, from `sysexits.h`.
const EX_USAGE: u8 = 64;

impl From<Outcome> for ExitCode {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Success => Self::SUCCESS,
            Outcome::Failure => Self::FAILURE,
            Outcome::Usage => Self::from(EX_USAGE),
        }
    }
}

/// A command that did not run, and why.
#[derive(Debug)]
pub struct Refusal {
    pub outcome: Outcome,
    pub diagnostic: Box<Diagnostic>,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.message.fmt(f)
    }
}

impl std::error::Error for Refusal {}

/// Parses `words` against `program` and runs the selected command, or prints help or the version.
///
/// # Errors
/// Returns a refusal for usage errors and for commands that are not implemented yet.
pub fn run<Cx>(
    program: &Program<Cx>,
    words: &[String],
    cx: &mut Cx,
    out: &mut dyn Write,
) -> Result<Outcome, Refusal> {
    let request =
        parse(program, words).map_err(|diagnostic| Refusal { outcome: Outcome::Usage, diagnostic })?;
    let written = match request {
        Request::Help(selected) => write!(out, "{}", help(program, &selected)).map(|()| Outcome::Success),
        Request::Version => {
            writeln!(out, "{} {}", program.root.name, program.version).map(|()| Outcome::Success)
        }
        Request::Run(selected, args) => match selected.last().action {
            Action::Run(run) => {
                (program.configure)(cx, &args)
                    .map_err(|diagnostic| Refusal { outcome: Outcome::Usage, diagnostic })?;
                run(cx, &args, out)
            }
            Action::Planned => {
                let msg = Msg::new().code(selected.invocation()).text(" is not available yet");
                let diagnostic = Box::new(Diagnostic::error(errors::CLI_UNAVAILABLE_COMMAND, msg));
                return Err(Refusal { outcome: Outcome::Failure, diagnostic });
            }
            Action::Group(_) => unreachable!("parse turns a bare group into a help request"),
        },
    };
    // Output that cannot be written, such as a closed pipe, fails the command without a report.
    Ok(written.unwrap_or(Outcome::Failure))
}
