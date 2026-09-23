//! Diagnostic identity, severity and source.

use std::borrow::Cow;
use std::fmt;

/// A binding path to a `lang.diag.Diag` value, such as `lang.errors.int.does_not_fit`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DiagId(pub Cow<'static, str>);

impl DiagId {
    /// Prefix under which compiler errors live; they are displayed relative to it.
    pub const COMPILER: &'static str = "lang.errors.";
    /// Separates the names of a binding path.
    pub const SEPARATOR: char = '.';

    /// An id known at compile time.
    #[must_use]
    pub const fn new(path: &'static str) -> Self {
        Self(Cow::Borrowed(path))
    }

    /// Whether this id lies in the namespace `group`, such as `lang.errors.syntax`.
    #[must_use]
    pub fn within(&self, group: &Self) -> bool {
        self.0.strip_prefix(&*group.0).is_some_and(|rest| rest.starts_with(Self::SEPARATOR))
    }
}

impl fmt::Display for DiagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.strip_prefix(Self::COMPILER).unwrap_or(&self.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Hint => "hint",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        })
    }
}

/// What produced a diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DiagSource {
    Compiler,
    Lint { group: Cow<'static, str> },
    Package { name: String, version: String },
}
