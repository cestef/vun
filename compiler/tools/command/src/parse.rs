//! Walking a command tree over the arguments.

use vun_diag::{ChildKind, DiagId, Diagnostic, Msg, errors};
use vun_suggest::closest;

use crate::{Args, Command, HELP, HELP_COMMAND, Param, ParamKind, Program, VERSION};

/// Prefix of long options.
pub(crate) const LONG: &str = "--";
/// Prefix of short options.
const SHORT: char = '-';
/// Separates a long option from its value in one argument.
const VALUE_SEPARATOR: char = '=';

pub enum Request<'p, Cx: 'static> {
    Run(Selected<'p, Cx>, Args),
    Help(Selected<'p, Cx>),
    Version,
}

/// The commands named on the command line, from the root down.
pub struct Selected<'p, Cx: 'static> {
    root: &'p Command<Cx>,
    below: Vec<&'p Command<Cx>>,
}

impl<'p, Cx> Selected<'p, Cx> {
    #[must_use]
    pub fn root(root: &'p Command<Cx>) -> Self {
        Self { root, below: Vec::new() }
    }

    /// The command that runs or whose help is shown.
    #[must_use]
    pub fn last(&self) -> &'p Command<Cx> {
        self.below.last().copied().unwrap_or(self.root)
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.below.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &'p Command<Cx>> {
        std::iter::once(self.root).chain(self.below.iter().copied())
    }

    /// The options the last command accepts: its own, then those of the groups above it, which
    /// apply to every command below them, then the built-in ones.
    pub fn options(&self) -> impl Iterator<Item = &'p Param> {
        let builtins: &'static [Param] = if self.is_root() { &[HELP, VERSION] } else { &[HELP] };
        let declared = self.iter().collect::<Vec<_>>().into_iter().rev().flat_map(|c| c.params);
        declared.filter(|p| p.kind != ParamKind::Positional).chain(builtins)
    }

    /// The command line selecting these commands: `vun debug cst`.
    #[must_use]
    pub fn invocation(&self) -> String {
        self.iter().map(|c| c.name).collect::<Vec<_>>().join(" ")
    }
}

impl<Cx> Program<Cx> {
    /// Resolves `words` to a request.
    ///
    /// # Errors
    /// Returns a usage diagnostic: unknown commands and options (with a suggestion when one is
    /// close), missing or unexpected arguments, options without values.
    pub fn parse(&self, words: &[String]) -> Result<Request<'_, Cx>, Box<Diagnostic>> {
        let mut selected = Selected::root(&self.root);
        let mut args = Args::default();
        let mut positional = 0;
        let mut help = false;
        let mut rest = words.iter();
        while let Some(word) = rest.next() {
            let command = selected.last();
            if let Some(option) = Given::of(word) {
                let param = selected.option(&option).ok_or_else(|| selected.unknown_option(&option))?;
                match (param.kind, option) {
                    _ if *param == HELP => help = true,
                    _ if *param == VERSION => return Ok(Request::Version),
                    (ParamKind::Flag, _) => args.flags.push(param.name),
                    (ParamKind::Option, Given::Long(_, Some(value))) => {
                        args.values.push((param.name, value.to_owned()));
                    }
                    (ParamKind::Option, _) => {
                        let value = rest.next().ok_or_else(|| param.missing_value())?;
                        args.values.push((param.name, value.clone()));
                    }
                    (ParamKind::Positional, _) => unreachable!("Selected::option finds options and flags"),
                }
            } else if !command.children().is_empty() {
                if word == HELP_COMMAND {
                    help = true;
                    continue;
                }
                let child = command.children().iter().find(|c| c.name == word);
                selected.below.push(child.ok_or_else(|| selected.unknown_command(word))?);
            } else {
                let param = command.positionals().nth(positional).ok_or_else(|| selected.unexpected(word))?;
                args.values.push((param.name, word.clone()));
                positional += 1;
            }
        }
        let command = selected.last();
        if help || !command.children().is_empty() {
            return Ok(Request::Help(selected));
        }
        if let Some(missing) = command.positionals().nth(positional) {
            return Err(selected.missing_argument(missing));
        }
        Ok(Request::Run(selected, args))
    }
}

/// An option as written.
enum Given<'a> {
    Long(&'a str, Option<&'a str>),
    Short(char),
}

impl<'a> Given<'a> {
    /// The option `arg` writes, if it is one.
    fn of(arg: &'a str) -> Option<Self> {
        if let Some(long) = arg.strip_prefix(LONG) {
            let (name, value) = match long.split_once(VALUE_SEPARATOR) {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            return Some(Self::Long(name, value));
        }
        // A single dash with several letters is read as a long option, so `-help` still resolves.
        let short = arg.strip_prefix(SHORT)?;
        let mut chars = short.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some(Self::Short(c)),
            (Some(_), Some(_)) => Some(Self::Long(short, None)),
            (None, _) => None,
        }
    }
}

impl<Cx> Command<Cx> {
    pub(crate) fn positionals(&self) -> impl Iterator<Item = &Param> {
        self.params.iter().filter(|p| p.kind == ParamKind::Positional)
    }
}

impl Param {
    fn missing_value(&self) -> Box<Diagnostic> {
        let msg = Msg::new().text("option ").code(format!("{LONG}{}", self.name)).text(" needs a value");
        let help = Msg::new().text("write ").code(format!("{LONG}{} <{}>", self.name, self.name));
        usage_error(errors::CLI_MISSING_VALUE, msg, help)
    }
}

impl<'p, Cx> Selected<'p, Cx> {
    /// The option or flag `given` names, among those of the selected commands.
    fn option(&self, given: &Given<'_>) -> Option<&'p Param> {
        self.options().find(|p| match *given {
            Given::Long(name, _) => p.name == name,
            Given::Short(c) => p.short == Some(c),
        })
    }

    fn see_help(&self) -> Msg {
        Msg::new().text("see ").code(format!("{} {LONG}{}", self.invocation(), HELP.name))
    }

    /// Help suggesting the closest of `candidates` to `given`, or pointing at the help page.
    fn suggestion<'a>(&self, given: &str, candidates: impl IntoIterator<Item = &'a str>) -> Msg {
        closest(given, candidates).map_or_else(|| self.see_help(), Msg::did_you_mean)
    }

    fn unknown_command(&self, name: &str) -> Box<Diagnostic> {
        let msg = Msg::new().text("unknown command ").code(name).text(" for ").code(self.invocation());
        let help = self.suggestion(name, self.last().children().iter().map(|c| c.name));
        usage_error(errors::CLI_UNKNOWN_COMMAND, msg, help)
    }

    fn unknown_option(&self, given: &Given<'_>) -> Box<Diagnostic> {
        let written = match *given {
            Given::Long(name, _) => format!("{LONG}{name}"),
            Given::Short(c) => format!("{SHORT}{c}"),
        };
        let msg =
            Msg::new().text("unknown option ").code(written.clone()).text(" for ").code(self.invocation());
        let names: Vec<String> = self.options().map(|p| format!("{LONG}{}", p.name)).collect();
        let help = self.suggestion(&written, names.iter().map(String::as_str));
        usage_error(errors::CLI_UNKNOWN_OPTION, msg, help)
    }

    fn missing_argument(&self, param: &Param) -> Box<Diagnostic> {
        let needed = format!("<{}>", param.name);
        let msg =
            Msg::new().code(self.invocation()).text(" needs ").code(needed).text(": ").text(param.about);
        let help = Msg::new().text("usage: ").code(self.usage());
        usage_error(errors::CLI_MISSING_ARGUMENT, msg, help)
    }

    fn unexpected(&self, word: &str) -> Box<Diagnostic> {
        let msg = Msg::new().text("unexpected argument ").code(word).text(" for ").code(self.invocation());
        usage_error(errors::CLI_UNEXPECTED_ARGUMENT, msg, self.see_help())
    }
}

fn usage_error(id: DiagId, msg: Msg, help: Msg) -> Box<Diagnostic> {
    Box::new(Diagnostic::error(id, msg).child(ChildKind::Help, help))
}

#[cfg(test)]
mod tests;
