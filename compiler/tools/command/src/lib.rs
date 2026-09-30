//! Declarative command trees: argument parsing, help pages, completions and usage errors as
//! diagnostics.
//!
//! A program is a tree of [`Command`]s declared as data. Parsing walks the tree, so names,
//! parameters, values and help are declared once, and every front end (`vun`, `cargo xtask`)
//! accepts the same syntax: `<command>... [--flag] [--option value] <argument>... [-- <word>...]`,
//! with `-h` (short help) and `--help` (long help, paged) everywhere, `help <command>...` in
//! groups, `-V`/`--version` on the root, and the global options of [`GLOBAL`].

mod complete;
mod help;
mod param;
mod parse;
mod run;

use std::io;
use std::process::ExitCode;

use vun_diag::Diagnostic;
pub use vun_term::Shell;

pub use complete::Dialect;
pub use param::{
    Args, Arity, COLOR, Catalog, FORMAT, GLOBAL, HELP, OptionSet, Param, ParamKind, PathKind, QUIET, VERSION,
    Values,
};
pub use parse::{Detail, Request, Selected};
pub use run::{Context, Diagnostics, HumanSink};

pub struct Program<Cx: 'static> {
    pub version: &'static str,
    pub root: Command<Cx>,
    /// Applies the options a command takes to the context, before the command runs.
    pub configure: Configure<Cx>,
}

/// Reads options into the context; an invalid value is a usage error.
pub type Configure<Cx> = fn(&mut Cx, &Args) -> Result<(), Box<Diagnostic>>;

/// A [`Configure`] for programs whose commands read their options themselves.
///
/// # Errors
/// Never.
pub fn unconfigured<Cx>(_: &mut Cx, _: &Args) -> Result<(), Box<Diagnostic>> {
    Ok(())
}

/// A command, declared as a constant: `Command::run("build", "build a program", build)
/// .params(&[FILE]).aliases(&["b"])`.
pub struct Command<Cx: 'static> {
    pub name: &'static str,
    pub about: &'static str,
    /// Other names it answers to, shown in help.
    pub aliases: &'static [&'static str],
    pub params: &'static [Param],
    /// Options it shares with other commands.
    pub sets: &'static [OptionSet],
    /// Command lines shown in long help.
    pub examples: &'static [Example],
    pub action: Action<Cx>,
}

pub enum Action<Cx: 'static> {
    Run(Run<Cx>),
    Group(&'static [Command<Cx>]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Example {
    /// The words after the program's name.
    pub line: &'static str,
    pub about: &'static str,
}

/// A command's implementation. Results go to the shell's standard output, status lines to its
/// standard error; diagnostics go through `Cx`.
pub type Run<Cx> = fn(&mut Cx, &Args, &mut Shell<'_>) -> io::Result<Outcome>;

impl<Cx> Command<Cx> {
    const fn new(name: &'static str, about: &'static str, action: Action<Cx>) -> Self {
        Self { name, about, aliases: &[], params: &[], sets: &[], examples: &[], action }
    }

    #[must_use]
    pub const fn run(name: &'static str, about: &'static str, run: Run<Cx>) -> Self {
        Self::new(name, about, Action::Run(run))
    }

    #[must_use]
    pub const fn group(name: &'static str, about: &'static str, children: &'static [Self]) -> Self {
        Self::new(name, about, Action::Group(children))
    }

    #[must_use]
    pub const fn params(self, params: &'static [Param]) -> Self {
        Self { params, ..self }
    }

    #[must_use]
    pub const fn sets(self, sets: &'static [OptionSet]) -> Self {
        Self { sets, ..self }
    }

    #[must_use]
    pub const fn aliases(self, aliases: &'static [&'static str]) -> Self {
        Self { aliases, ..self }
    }

    #[must_use]
    pub const fn examples(self, examples: &'static [Example]) -> Self {
        Self { examples, ..self }
    }

    #[must_use]
    pub fn children(&self) -> &'static [Self] {
        match self.action {
            Action::Group(children) => children,
            Action::Run(_) => &[],
        }
    }

    /// Whether `word` names it.
    #[must_use]
    pub fn answers(&self, word: &str) -> bool {
        self.name == word || self.aliases.contains(&word)
    }

    /// The child `word` names.
    #[must_use]
    pub fn child(&self, word: &str) -> Option<&'static Self> {
        self.children().iter().find(|c| c.answers(word))
    }

    pub(crate) fn positionals(&self) -> impl Iterator<Item = &Param> {
        self.params.iter().filter(|p| p.is_positional())
    }
}

impl<Cx> Program<Cx> {
    /// The command line selecting the command at `path` below the root, such as `vun pkg
    /// add`, for messages that suggest it; `None` if no command is there.
    #[must_use]
    pub fn invocation(&self, path: &[&str]) -> Option<String> {
        let mut command = &self.root;
        let mut line = command.name.to_owned();
        for name in path {
            command = command.child(name)?;
            line.push(' ');
            line.push_str(command.name);
        }
        Some(line)
    }
}

/// The group command that shows help for the commands after it.
pub const HELP_COMMAND: &str = "help";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Failure,
    /// Command-line misuse, reported as `EX_USAGE` from `sysexits.h`.
    Usage,
    /// The status a program the command ran exited with, passed on.
    Exited(u8),
}

/// Exit status of command-line misuse, from `sysexits.h`.
const EX_USAGE: u8 = 64;

impl From<Outcome> for ExitCode {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Success => Self::SUCCESS,
            Outcome::Failure => Self::FAILURE,
            Outcome::Usage => Self::from(EX_USAGE),
            Outcome::Exited(status) => Self::from(status),
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
