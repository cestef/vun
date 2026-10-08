//! Options as written: reading one from a word, recording its value, and reading the global
//! ones ahead of parsing.

use vun_diag::Diagnostic;
use vun_span::Place;
use vun_term::Percent;

use super::{LONG, OPTIONS_END, SHORT};
use crate::{Args, Arity, GLOBAL, Param, ParamKind, Program, Values};

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
            drop(args.take(param, given, given.value(param, &mut rest)));
        }
        drop(args.fill(GLOBAL.params.iter()));
        args
    }
}

impl Args {
    /// Records `param` as `given`: a flag, which takes no value, or an option with `value`.
    pub(super) fn take(
        &mut self,
        param: &Param,
        given: Given<'_>,
        value: Option<&str>,
    ) -> Result<(), Box<Diagnostic>> {
        match (param.kind, given, value) {
            (ParamKind::Flag, Given::Long(_, Some(value)), _) => Err(param.unexpected_value(value)),
            (ParamKind::Flag, Given::Repeated(_, times), _) => {
                self.flags.extend(std::iter::repeat_n(param.name, times));
                Ok(())
            }
            (ParamKind::Flag, ..) => {
                self.flags.push(param.name);
                Ok(())
            }
            (_, _, Some(value)) => self.give(param, value),
            (_, _, None) => Err(param.missing_value()),
        }
    }

    /// Gives each of `params` the command line left out the value of its environment variable,
    /// when that is set and not empty.
    pub(super) fn fill<'p>(
        &mut self,
        params: impl Iterator<Item = &'p Param>,
    ) -> Result<(), Box<Diagnostic>> {
        let unset: Vec<&Param> = params.filter(|p| self.value(p).is_none()).collect();
        for param in unset {
            if let Some(value) = param.env.and_then(|name| std::env::var(name).ok()).filter(|v| !v.is_empty())
            {
                self.give(param, &value)?;
            }
        }
        Ok(())
    }

    /// Records `value` for `param`, checking it against the parameter's values. A parameter
    /// taking one value keeps the last given.
    pub(super) fn give(&mut self, param: &Param, value: &str) -> Result<(), Box<Diagnostic>> {
        let names = param.choices().unwrap_or_default();
        let valid = match param.values {
            Values::Choice(_) | Values::Catalog(_) => names.contains(&value),
            Values::Count => value.parse::<u64>().is_ok(),
            Values::Place => value.parse::<Place>().is_ok(),
            Values::Percent => value.parse::<Percent>().is_ok(),
            Values::Text | Values::Path(_) => true,
        };
        if !valid {
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
    /// One short option written several times in a word, such as `-vv`.
    Repeated(char, usize),
}

impl<'a> Given<'a> {
    /// Whether it is written for `param`.
    pub(super) fn names(&self, param: &Param) -> bool {
        match *self {
            Self::Long(name, _) => param.name == name,
            Self::Short(c) | Self::Repeated(c, _) => param.short == Some(c),
        }
    }

    /// The value it gives `param`: none for a flag, the one after `=`, or else the next word.
    pub(super) fn value<'w>(
        self,
        param: &Param,
        rest: &mut impl Iterator<Item = &'w String>,
    ) -> Option<&'w str>
    where
        'a: 'w,
    {
        match (param.kind, self) {
            (ParamKind::Flag, _) => None,
            (_, Self::Long(_, Some(value))) => Some(value),
            _ => rest.next().map(String::as_str),
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
        // A single dash with several letters is read as a long option, so `-help` still resolves;
        // one letter repeated, as `-vv`, is that option given as often.
        let short = arg.strip_prefix(SHORT)?;
        let mut chars = short.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some(Self::Short(c)),
            (Some(c), Some(_)) if short.chars().all(|d| d == c) => {
                Some(Self::Repeated(c, short.chars().count()))
            }
            (Some(_), Some(_)) => Some(Self::Long(short, None)),
            (None, _) => None,
        }
    }
}
