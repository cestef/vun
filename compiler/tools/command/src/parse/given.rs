//! Options as written: reading one from a word, recording its value, and reading the global
//! ones ahead of parsing.

use vun_diag::Diagnostic;

use super::{LONG, OPTIONS_END};
use crate::{Args, Arity, GLOBAL, Param, ParamKind, Program};

/// Prefix of short options.
pub(super) const SHORT: char = '-';
/// Separates a long option from its value in one argument.
const VALUE_SEPARATOR: char = '=';

impl<Cx> Program<Cx> {
    /// The global options `words` give, or their variables give, read before the command line
    /// is parsed, so that even its usage errors are shown as they ask. A value that is not a
    /// choice is left for parsing to report.
    #[must_use]
    pub fn globals(&self, words: &[String]) -> Args {
        let mut args = Args::default();
        let mut rest = words.iter().take_while(|w| *w != OPTIONS_END);
        while let Some(word) = rest.next() {
            let Some(given) = Given::of(word) else { continue };
            let Some(param) = GLOBAL.params.iter().find(|p| given.names(p)) else { continue };
            match (param.kind, given) {
                (ParamKind::Flag, _) => args.flags.push(param.name),
                (_, Given::Long(_, Some(value))) => drop(args.give(param, value)),
                _ => {
                    if let Some(value) = rest.next() {
                        drop(args.give(param, value));
                    }
                }
            }
        }
        let unset: Vec<&Param> = GLOBAL.params.iter().filter(|p| args.value(p).is_none()).collect();
        for param in unset {
            if let Some(value) = param.env.and_then(|name| std::env::var(name).ok()) {
                drop(args.give(param, &value));
            }
        }
        args
    }
}

impl Args {
    /// Records `value` for `param`, checking it against the parameter's choices. A parameter
    /// taking one value keeps the last given.
    pub(super) fn give(&mut self, param: &Param, value: &str) -> Result<(), Box<Diagnostic>> {
        if let Some(names) = param.choices()
            && !names.contains(&value)
        {
            return Err(param.invalid_value(value, &names));
        }
        if matches!(param.kind, ParamKind::Option(Arity::One | Arity::Optional)) {
            self.values.retain(|(name, _)| *name != param.name);
        }
        self.values.push((param.name, value.to_owned()));
        Ok(())
    }
}

/// An option as written.
#[derive(Clone, Copy)]
pub(super) enum Given<'a> {
    Long(&'a str, Option<&'a str>),
    Short(char),
}

impl<'a> Given<'a> {
    /// Whether it is written for `param`.
    pub(super) fn names(&self, param: &Param) -> bool {
        match *self {
            Self::Long(name, _) => param.name == name,
            Self::Short(c) => param.short == Some(c),
        }
    }

    /// The option `arg` writes, if it is one.
    pub(super) fn of(arg: &'a str) -> Option<Self> {
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
