//! Walking a command tree over the arguments.

use vun_diag::{ChildKind, DiagId, Diagnostic, Msg, errors};
use vun_suggest::closest;

use given::Given;

use crate::{
    Args, Arity, Catalog, Command, GLOBAL, HELP, HELP_COMMAND, Param, ParamKind, Program, VERSION, Values,
};

/// Prefix of long options.
pub(crate) const LONG: &str = "--";
/// Prefix of short options.
pub(crate) const SHORT: char = '-';
/// The title of a command's own options in help.
pub(crate) const OWN: &str = "Options";
/// Ends the options: every word after it is positional.
pub(crate) const OPTIONS_END: &str = "--";

/// How much help was asked for: `-h` shows the page, `--help` and `help` show everything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Short,
    Long,
}

pub enum Request<'p, Cx: 'static> {
    Run(Selected<'p, Cx>, Args),
    /// The help page of the selected command, with the options given with it.
    Help(Selected<'p, Cx>, Args, Detail),
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

    /// The command `child` below these.
    #[must_use]
    pub fn then(&self, child: &'p Command<Cx>) -> Self {
        let below = self.below.iter().copied().chain([child]).collect();
        Self { root: self.root, below }
    }

    /// The names of the commands below the root: `["pkg", "add"]`.
    #[must_use]
    pub fn path(&self) -> Vec<&'static str> {
        self.below.iter().map(|c| c.name).collect()
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

    /// The options the last command accepts, by section: its own, then those of the groups
    /// above it, which apply to every command below them; then the sets these commands take;
    /// then the global ones.
    #[must_use]
    pub fn sections(&self) -> Vec<(&'static str, Vec<&'p Param>)> {
        let chain: Vec<_> = self.iter().collect::<Vec<_>>().into_iter().rev().collect();
        let own = chain.iter().flat_map(|c| c.params).filter(|p| !p.is_positional()).collect();
        let sets = chain.iter().flat_map(|c| c.sets).map(|set| (set.title, set.params.iter().collect()));
        let version: &'static [Param] = if self.is_root() { &[VERSION] } else { &[] };
        let global = (GLOBAL.title, GLOBAL.params.iter().chain(version).collect());
        std::iter::once((OWN, own)).chain(sets).chain([global]).collect()
    }

    /// Every option the last command accepts.
    pub fn options(&self) -> impl Iterator<Item = &'p Param> {
        self.sections().into_iter().flat_map(|(_, params)| params)
    }

    /// The command line selecting these commands: `vun debug dump cst`.
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
    /// close), values outside a parameter's choices, missing or unexpected arguments, options
    /// without values.
    pub fn parse(&self, words: &[String]) -> Result<Request<'_, Cx>, Box<Diagnostic>> {
        let mut selected = Selected::root(&self.root);
        let mut args = Args::default();
        let mut positional = 0;
        let mut help = None;
        let mut rest = words.iter();
        let mut options_end = false;
        while let Some(word) = rest.next() {
            let command = selected.last();
            if word == OPTIONS_END && !options_end {
                options_end = true;
                continue;
            }
            if let Some(option) = Given::of(word).filter(|_| !options_end) {
                let param = selected.option(&option).ok_or_else(|| selected.unknown_option(&option))?;
                match (param.kind, option) {
                    _ if param.is(&HELP) => {
                        let detail =
                            if matches!(option, Given::Short(_)) { Detail::Short } else { Detail::Long };
                        help = Some(detail);
                    }
                    _ if param.is(&VERSION) => return Ok(Request::Version),
                    (ParamKind::Positional(_), _) => unreachable!("Selected::option finds options and flags"),
                    _ => args.take(param, option, option.value(param, &mut rest))?,
                }
            } else if !command.children().is_empty() && !options_end {
                if word == HELP_COMMAND {
                    help = Some(Detail::Long);
                    continue;
                }
                selected.below.push(command.child(word).ok_or_else(|| selected.unknown_command(word))?);
            } else {
                let param = command.positionals().nth(positional).ok_or_else(|| selected.unexpected(word))?;
                args.give(param, word)?;
                if param.kind != ParamKind::Positional(Arity::Many) {
                    positional += 1;
                }
            }
        }
        selected.defaults(&mut args)?;
        let command = selected.last();
        if let Some(detail) = help {
            return Ok(Request::Help(selected, args, detail));
        }
        if !command.children().is_empty() {
            return Ok(Request::Help(selected, args, Detail::Short));
        }
        if let Some(missing) =
            command.positionals().filter(|p| p.is_required()).find(|p| args.value(p).is_none())
        {
            return Err(selected.missing_argument(missing));
        }
        Ok(Request::Run(selected, args))
    }
}

impl Param {
    fn invalid_value(&self, value: &str, names: &[&str]) -> Box<Diagnostic> {
        let msg = Msg::new().code(value).text(" is not a value of ").code(self.written());
        let all = || match self.values {
            Values::Catalog(Catalog { what, declared_in, .. }) => {
                Msg::new().text("choose ").text(what).text(", as ").code(declared_in).text(" declares them")
            }
            _ => Msg::new().text("choose one of ").code_list(names.iter().copied()),
        };
        let help = closest(value, names.iter().copied()).map_or_else(all, Msg::did_you_mean);
        usage_error(errors::CLI_INVALID_VALUE, msg, help)
    }

    fn unexpected_value(&self, value: &str) -> Box<Diagnostic> {
        let msg = Msg::new().text("flag ").code(self.long_form()).text(" takes no value, given ").code(value);
        let help = Msg::new().text("write ").code(self.long_form());
        usage_error(errors::CLI_UNEXPECTED_ARGUMENT, msg, help)
    }

    fn missing_value(&self) -> Box<Diagnostic> {
        let msg = Msg::new().text("option ").code(self.long_form()).text(" needs a value");
        let help = Msg::new().text("write ").code(self.long());
        usage_error(errors::CLI_MISSING_VALUE, msg, help)
    }
}

impl<'p, Cx> Selected<'p, Cx> {
    /// The option or flag `given` names, among those of the selected commands.
    fn option(&self, given: &Given<'_>) -> Option<&'p Param> {
        self.options().find(|p| given.names(p))
    }

    /// Fills the options the command line left out from their environment variables.
    fn defaults(&self, args: &mut Args) -> Result<(), Box<Diagnostic>> {
        args.fill(self.options().chain(self.last().positionals()))
    }

    fn see_help(&self) -> Msg {
        Msg::new().text("see ").code([self.invocation(), HELP.long_form()].join(" "))
    }

    /// Help suggesting the closest of `candidates` to `given`, or pointing at the help page.
    fn suggestion<'a>(&self, given: &str, candidates: impl IntoIterator<Item = &'a str>) -> Msg {
        closest(given, candidates).map_or_else(|| self.see_help(), Msg::did_you_mean)
    }

    fn unknown_command(&self, name: &str) -> Box<Diagnostic> {
        let msg = Msg::new().text("unknown command ").code(name).text(" for ").code(self.invocation());
        let names = self
            .last()
            .children()
            .iter()
            .flat_map(|c| std::iter::once(c.name).chain(c.aliases.iter().copied()));
        let help = self.suggestion(name, names);
        usage_error(errors::CLI_UNKNOWN_COMMAND, msg, help)
    }

    fn unknown_option(&self, given: &Given<'_>) -> Box<Diagnostic> {
        let written = match *given {
            Given::Long(name, _) => format!("{LONG}{name}"),
            Given::Short(c) => format!("{SHORT}{c}"),
        };
        let msg =
            Msg::new().text("unknown option ").code(written.clone()).text(" for ").code(self.invocation());
        let names: Vec<String> = self.options().map(Param::long_form).collect();
        let help = self.suggestion(&written, names.iter().map(String::as_str));
        usage_error(errors::CLI_UNKNOWN_OPTION, msg, help)
    }

    fn missing_argument(&self, param: &Param) -> Box<Diagnostic> {
        let needed = param.value_form();
        let msg =
            Msg::new().code(self.invocation()).text(" needs ").code(needed).text(": ").text(param.about);
        let help = Msg::new().text("usage: ").code(self.usage().to_string());
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

mod given;
#[cfg(test)]
mod tests;
