//! The rules of a grammar, as the functions of `grammar.js` build them.

use std::fmt::{self, Write};

use vun_data::Value;
use vun_data::json::Json;

/// A rule of a tree-sitter grammar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Another rule, by name: `$.name`.
    Symbol(String),
    /// A token spelled so.
    Text(String),
    /// A token matching a regular expression, in the dialect tree-sitter reads.
    Pattern(String),
    Seq(Vec<Rule>),
    Choice(Vec<Rule>),
    Repeat(Box<Rule>),
    Optional(Box<Rule>),
    Field(String, Box<Rule>),
    /// A precedence resolving conflicts while the parser is generated.
    Prec(Assoc, i32, Box<Rule>),
    /// A preference among parses that all fit, while parsing.
    Dynamic(i32, Box<Rule>),
    /// The rule as one token.
    Token(Box<Rule>),
    /// The rule, its node named as the second, a named rule.
    Alias(Box<Rule>, String),
}

/// How a rule under a precedence groups with its own level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assoc {
    None,
    Left,
    Right,
}

impl Rule {
    #[must_use]
    pub fn symbol(name: &str) -> Self {
        Self::Symbol(name.to_owned())
    }

    #[must_use]
    pub fn text(text: &str) -> Self {
        Self::Text(text.to_owned())
    }

    /// `rules` in sequence, the one itself.
    #[must_use]
    pub fn seq(mut rules: Vec<Self>) -> Self {
        if rules.len() == 1 { rules.remove(0) } else { Self::Seq(rules) }
    }

    /// One of `rules`, the one itself.
    #[must_use]
    pub fn choice(mut rules: Vec<Self>) -> Self {
        if rules.len() == 1 { rules.remove(0) } else { Self::Choice(rules) }
    }

    #[must_use]
    pub fn repeat(self) -> Self {
        Self::Repeat(Box::new(self))
    }

    #[must_use]
    pub fn optional(self) -> Self {
        Self::Optional(Box::new(self))
    }

    #[must_use]
    pub fn field(self, name: &str) -> Self {
        Self::Field(name.to_owned(), Box::new(self))
    }

    #[must_use]
    pub fn prec(self, assoc: Assoc, level: i32) -> Self {
        Self::Prec(assoc, level, Box::new(self))
    }

    #[must_use]
    pub fn dynamic(self, level: i32) -> Self {
        Self::Dynamic(level, Box::new(self))
    }

    /// This rule, its node named `name`.
    #[must_use]
    pub fn alias(self, name: &str) -> Self {
        Self::Alias(Box::new(self), name.to_owned())
    }

    #[must_use]
    pub fn token(self) -> Self {
        Self::Token(Box::new(self))
    }
}

/// `name(a, b, ...)`.
fn call(f: &mut fmt::Formatter<'_>, name: &str, rules: &[Rule]) -> fmt::Result {
    f.write_str(name)?;
    f.write_char('(')?;
    for (i, r) in rules.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{r}")?;
    }
    f.write_char(')')
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symbol(name) => write!(f, "$.{name}"),
            Self::Text(text) => write!(f, "{}", Json(&Value::from(text.clone()))),
            Self::Pattern(pattern) => write!(f, "/{}/", pattern.replace('/', r"\/")),
            Self::Seq(rules) => call(f, "seq", rules),
            Self::Choice(rules) => call(f, "choice", rules),
            Self::Repeat(rule) => call(f, "repeat", std::slice::from_ref(rule)),
            Self::Optional(rule) => call(f, "optional", std::slice::from_ref(rule)),
            Self::Token(rule) => call(f, "token", std::slice::from_ref(rule)),
            Self::Field(name, rule) => write!(f, "field({}, {rule})", Json(&Value::from(name.clone()))),
            Self::Alias(rule, name) => write!(f, "alias({rule}, $.{name})"),
            Self::Dynamic(level, rule) => write!(f, "prec.dynamic({level}, {rule})"),
            Self::Prec(assoc, level, rule) => {
                let name = match assoc {
                    Assoc::None => "prec",
                    Assoc::Left => "prec.left",
                    Assoc::Right => "prec.right",
                };
                write!(f, "{name}({level}, {rule})")
            }
        }
    }
}
