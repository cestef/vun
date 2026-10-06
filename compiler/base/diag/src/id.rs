//! Diagnostic identity, severity and source.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use vun_codec::{Decode, Encode};
use vun_intern::{Internable, Interned, Pool, Symbol};

use crate::errors::Lang;

/// A binding path to a `lang.diag.Diag` value, such as `lang.errors.int.does_not_fit`: a
/// compiler id, an id of a tool of the repository, or the path of a lint a package defines.
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub enum DiagId {
    /// Declared in `lang.errors`, which the compiler reports.
    Lang(Lang),
    /// Declared by a tool written in Rust, such as `xtask.lines.oversized`.
    Tool(&'static str),
    /// Defined at run time by a package: a lint or a group of lints.
    Package(Interned<Segments>),
}

/// The names of a binding path, outermost first.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Segments(Vec<Symbol>);

static SEGMENTS: Pool<Segments> = Pool::new();

impl Internable for Segments {
    const POOL: &'static str = "diag_path";

    fn pool() -> &'static Pool<Self> {
        &SEGMENTS
    }
}

impl DiagId {
    /// Prefix under which compiler errors live; they are displayed relative to it.
    pub const COMPILER: &'static str = "lang.errors.";
    /// Separates the names of a binding path.
    pub const SEPARATOR: char = '.';
    /// The extension of explanation pages: an id's page is the file its binding path names on
    /// disk, as a namespace's source is, with this extension (spec 10, section 1).
    pub const PAGE: &'static str = "md";

    /// The id of a tool of the repository, known at compile time.
    #[must_use]
    pub const fn new(path: &'static str) -> Self {
        Self::Tool(path)
    }

    /// The id of the binding path of `names`: a compiler id when `lang.errors` declares it.
    #[must_use]
    pub fn package(names: impl IntoIterator<Item = Symbol>) -> Self {
        let names: Vec<Symbol> = names.into_iter().collect();
        let same = |id: &&Lang| id.path().split(Self::SEPARATOR).eq(names.iter().map(|n| n.as_str()));
        match Lang::ALL.iter().find(same) {
            Some(&id) => Self::Lang(id),
            None => Self::Package(SEGMENTS.intern(Segments(names))),
        }
    }

    /// The id of a binding path read as text, such as a name given on a command line.
    #[must_use]
    pub fn parse(path: &str) -> Self {
        Self::package(path.split(Self::SEPARATOR).map(Symbol::intern))
    }

    /// The compiler error written `relative` to [`Self::COMPILER`], as annotations and
    /// displays name it: `int.does_not_fit`.
    #[must_use]
    pub fn compiler(relative: &str) -> Self {
        Self::parse(&[Self::COMPILER, relative].concat())
    }

    /// The names of the binding path.
    #[must_use]
    pub fn segments(&self) -> Vec<&str> {
        match self {
            Self::Lang(id) => id.path().split(Self::SEPARATOR).collect(),
            Self::Tool(path) => path.split(Self::SEPARATOR).collect(),
            Self::Package(names) => names.get().0.iter().map(|n| n.as_str()).collect(),
        }
    }

    /// The binding path as text, names separated by [`Self::SEPARATOR`].
    #[must_use]
    pub fn path(&self) -> Cow<'static, str> {
        match self {
            Self::Lang(id) => Cow::Borrowed(id.path()),
            Self::Tool(path) => Cow::Borrowed(path),
            Self::Package(names) => {
                let names: Vec<&str> = names.get().0.iter().map(|n| n.as_str()).collect();
                Cow::Owned(names.join(&Self::SEPARATOR.to_string()))
            }
        }
    }

    /// Whether this id is `group` or lies in its namespace, such as `lang.errors.syntax`.
    #[must_use]
    pub fn within(&self, group: &Self) -> bool {
        let mut names = self.segments().into_iter();
        group.segments().into_iter().all(|name| names.next() == Some(name))
    }
}

/// By binding path, whatever declares the id: a tool constant and a package id naming the same
/// path are one id.
impl PartialEq for DiagId {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Lang(a), Self::Lang(b)) => a == b,
            _ => self.path() == other.path(),
        }
    }
}

impl Eq for DiagId {}

impl Hash for DiagId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path().hash(state);
    }
}

impl PartialOrd for DiagId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// By binding path, so that diagnostics are listed in the same order whatever declares them.
impl Ord for DiagId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path().cmp(&other.path())
    }
}

impl fmt::Display for DiagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path();
        f.write_str(path.strip_prefix(Self::COMPILER).unwrap_or(&path))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
}

impl Severity {
    /// How renderers style its title.
    #[must_use]
    pub const fn style(self) -> vun_term::Style {
        match self {
            Self::Hint => vun_term::Style::Help,
            Self::Info => vun_term::Style::Accent,
            Self::Warning => vun_term::Style::Warning,
            Self::Error => vun_term::Style::Error,
        }
    }
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum DiagSource {
    Compiler,
    Lint { group: Symbol },
    Package { name: String, version: String },
}
