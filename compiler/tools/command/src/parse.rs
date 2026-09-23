//! Walking a command tree over the arguments.

use overt_diag::suggest::closest;
use overt_diag::{ChildKind, DiagId, Diagnostic, Msg, errors};

use crate::{Args, Command, HELP, HELP_COMMAND, Param, ParamKind, Program, VERSION};

/// Prefix of long options.
const LONG: &str = "--";
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

    /// The command line selecting these commands: `overt dump cst`.
    #[must_use]
    pub fn invocation(&self) -> String {
        self.iter().map(|c| c.name).collect::<Vec<_>>().join(" ")
    }
}

/// Resolves `words` to a request.
///
/// # Errors
/// Returns a usage diagnostic: unknown commands and options (with a suggestion when one is
/// close), missing or unexpected arguments, options without values.
pub fn parse<'p, Cx>(program: &'p Program<Cx>, words: &[String]) -> Result<Request<'p, Cx>, Box<Diagnostic>> {
    let mut selected = Selected::root(&program.root);
    let mut args = Args::default();
    let mut positional = 0;
    let mut help = false;
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        let command = selected.last();
        if let Some(option) = option(word) {
            let param = find_option(&selected, &option).ok_or_else(|| unknown_option(&selected, &option))?;
            match (param.kind, option) {
                _ if param.name == HELP.name => help = true,
                _ if param.name == VERSION.name => return Ok(Request::Version),
                (ParamKind::Flag, _) => args.flags.push(param.name),
                (ParamKind::Option, Given::Long(_, Some(value))) => {
                    args.values.push((param.name, value.to_owned()));
                }
                (ParamKind::Option, _) => {
                    let value = rest.next().ok_or_else(|| missing_value(param))?;
                    args.values.push((param.name, value.clone()));
                }
                (ParamKind::Positional, _) => unreachable!("find_option returns options and flags"),
            }
        } else if !command.children().is_empty() {
            if word == HELP_COMMAND {
                help = true;
                continue;
            }
            let child = command.children().iter().find(|c| c.name == word);
            selected.below.push(child.ok_or_else(|| unknown_command(&selected, word))?);
        } else {
            let param = positionals(command).nth(positional).ok_or_else(|| unexpected(&selected, word))?;
            args.values.push((param.name, word.clone()));
            positional += 1;
        }
    }
    let command = selected.last();
    if help || !command.children().is_empty() {
        return Ok(Request::Help(selected));
    }
    if let Some(missing) = positionals(command).nth(positional) {
        return Err(missing_argument(&selected, missing));
    }
    Ok(Request::Run(selected, args))
}

/// An option as written.
enum Given<'a> {
    Long(&'a str, Option<&'a str>),
    Short(char),
}

fn option(arg: &str) -> Option<Given<'_>> {
    if let Some(long) = arg.strip_prefix(LONG) {
        let (name, value) = match long.split_once(VALUE_SEPARATOR) {
            Some((name, value)) => (name, Some(value)),
            None => (long, None),
        };
        return Some(Given::Long(name, value));
    }
    // A single dash with several letters is read as a long option, so `-help` still resolves.
    let short = arg.strip_prefix(SHORT)?;
    let mut chars = short.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(Given::Short(c)),
        (Some(_), Some(_)) => Some(Given::Long(short, None)),
        (None, _) => None,
    }
}

pub(crate) fn positionals<Cx>(command: &Command<Cx>) -> impl Iterator<Item = &Param> {
    command.params.iter().filter(|p| p.kind == ParamKind::Positional)
}

fn find_option<'p, Cx>(selected: &Selected<'p, Cx>, given: &Given<'_>) -> Option<&'p Param> {
    selected.options().find(|p| match *given {
        Given::Long(name, _) => p.name == name,
        Given::Short(c) => p.short == Some(c),
    })
}

fn usage_error(id: DiagId, msg: Msg, help: Msg) -> Box<Diagnostic> {
    Box::new(Diagnostic::error(id, msg).child(ChildKind::Help, help))
}

fn see_help<Cx>(selected: &Selected<'_, Cx>) -> Msg {
    Msg::new().text("see ").code(format!("{} {LONG}{}", selected.invocation(), HELP.name))
}

/// Help suggesting the closest of `candidates` to `given`, or pointing at the help page.
fn suggestion<'a, Cx>(
    given: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    selected: &Selected<'_, Cx>,
) -> Msg {
    match closest(given, candidates) {
        Some(near) => Msg::new().text("did you mean ").code(near).text("?"),
        None => see_help(selected),
    }
}

fn unknown_command<Cx>(selected: &Selected<'_, Cx>, name: &str) -> Box<Diagnostic> {
    let msg = Msg::new().text("unknown command ").code(name).text(" for ").code(selected.invocation());
    let help = suggestion(name, selected.last().children().iter().map(|c| c.name), selected);
    usage_error(errors::CLI_UNKNOWN_COMMAND, msg, help)
}

fn unknown_option<Cx>(selected: &Selected<'_, Cx>, given: &Given<'_>) -> Box<Diagnostic> {
    let written = match *given {
        Given::Long(name, _) => format!("{LONG}{name}"),
        Given::Short(c) => format!("{SHORT}{c}"),
    };
    let msg =
        Msg::new().text("unknown option ").code(written.clone()).text(" for ").code(selected.invocation());
    let names: Vec<String> = selected.options().map(|p| format!("{LONG}{}", p.name)).collect();
    let help = suggestion(&written, names.iter().map(String::as_str), selected);
    usage_error(errors::CLI_UNKNOWN_OPTION, msg, help)
}

fn missing_value(param: &Param) -> Box<Diagnostic> {
    let msg = Msg::new().text("option ").code(format!("{LONG}{}", param.name)).text(" needs a value");
    let help = Msg::new().text("write ").code(format!("{LONG}{} <{}>", param.name, param.name));
    usage_error(errors::CLI_MISSING_VALUE, msg, help)
}

fn missing_argument<Cx>(selected: &Selected<'_, Cx>, param: &Param) -> Box<Diagnostic> {
    let needed = format!("<{}>", param.name);
    let msg =
        Msg::new().code(selected.invocation()).text(" needs ").code(needed).text(": ").text(param.about);
    let help = Msg::new().text("usage: ").code(crate::help::usage(selected));
    usage_error(errors::CLI_MISSING_ARGUMENT, msg, help)
}

fn unexpected<Cx>(selected: &Selected<'_, Cx>, word: &str) -> Box<Diagnostic> {
    let msg = Msg::new().text("unexpected argument ").code(word).text(" for ").code(selected.invocation());
    usage_error(errors::CLI_UNEXPECTED_ARGUMENT, msg, see_help(selected))
}

#[cfg(test)]
mod tests;
