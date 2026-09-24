//! Test harnesses: corpus expectations, snapshots, blessing.
//!
//! Rejection tests annotate the diagnostics they expect next to the code that causes them:
//!
//! ```text
//! x: u8 = 300;       //~ int.does_not_fit
//! y := x + ;
//! //~^ syntax.expected_expression
//! ```
//!
//! `//~ id` expects `id` on its own line; each `^` moves the expectation one line up. A run passes
//! when the reported `(line, id)` pairs equal the expected ones exactly, so cascades fail tests.
//! An annotation may also pin the column, `//~ id @12`, and require fragments of the rendered
//! diagnostic (its message, notes and help), `//~ id "escape it"`; a diagnostic that exists but
//! fails these checks is reported as differing.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use vun_fs::{Edit, Tree};

mod page;

pub use page::{Block, Page};

/// The repository root, located from this crate's own place in the workspace.
///
/// # Panics
/// Panics if the crate was moved out of `compiler/tools`.
#[must_use]
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(REPO_DEPTH)
        .expect("harness lives in the repository")
        .to_path_buf()
}

/// Depth of this crate below the repository root: `compiler/tools/harness`.
const REPO_DEPTH: usize = 3;

/// Programs that check without a diagnostic.
pub const ACCEPT: &str = "corpus/accept";
/// Programs annotated with the diagnostics they cause.
pub const REJECT: &str = "corpus/reject";
/// Programs written ahead of the libraries they use: they parse, and are listed by the names they
/// wait for until they check clean.
pub const PLANNED: &str = "corpus/planned";
/// The directories of Vun sources that must lex, parse and lower cleanly: the libraries and every
/// program meant to check.
pub const SOURCES: &[&str] = &["lib", ACCEPT, PLANNED];

/// Marker that starts an expectation inside a line comment.
pub const MARKER: &str = "//~";
/// Environment variable that makes snapshot checks overwrite instead of compare.
pub const BLESS_VAR: &str = "VUN_BLESS";

/// A diagnostic expected or reported on a one-based line.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct At {
    pub line: u32,
    pub id: String,
}

/// Marks the column of an annotation: `@12`.
pub const COLUMN: char = '@';
/// Delimits a required fragment of the rendered diagnostic: `"escape it"`.
pub const FRAGMENT: char = '"';

/// An annotation: where and what, and optionally the column and fragments of the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expected {
    pub at: At,
    pub col: Option<u32>,
    pub fragments: Vec<String>,
}

/// A diagnostic as a run reported it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reported {
    pub at: At,
    pub col: u32,
    /// The message with its notes and help, as rendered.
    pub text: String,
}

impl Expected {
    fn matches(&self, reported: &Reported) -> bool {
        reported.at == self.at
            && self.col.is_none_or(|col| col == reported.col)
            && self.fragments.iter().all(|f| reported.text.contains(f.as_str()))
    }
}

/// Why an annotation cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// The `^` marks point above the first line.
    AboveFile { line: u32 },
    /// A `@` not followed by a column number.
    Column { line: u32 },
    /// A fragment without its closing quote.
    Fragment { line: u32 },
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::AboveFile { line } => write!(f, "line {line}: `^` points above the first line"),
            Self::Column { line } => write!(f, "line {line}: `@` is not followed by a column"),
            Self::Fragment { line } => write!(f, "line {line}: a fragment has no closing quote"),
        }
    }
}

impl std::error::Error for Malformed {}

impl Expected {
    /// The annotations written in `text`.
    ///
    /// # Errors
    /// Returns the first annotation that cannot be read.
    pub fn all(text: &str) -> Result<Vec<Self>, Malformed> {
        let mut found = Vec::new();
        for (line, number) in text.lines().zip(1u32..) {
            let Some(start) = line.find(MARKER) else { continue };
            let rest = &line[start + MARKER.len()..];
            let up = rest.chars().take_while(|&c| c == '^').count();
            let target = u32::try_from(up)
                .ok()
                .and_then(|up| number.checked_sub(up))
                .filter(|&l| l > 0)
                .ok_or(Malformed::AboveFile { line: number })?;
            let rest = rest.trim_start_matches('^').trim_start();
            let (id, mut rest) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            let mut expected =
                Self { at: At { line: target, id: id.to_owned() }, col: None, fragments: Vec::new() };
            loop {
                rest = rest.trim_start();
                if let Some(after) = rest.strip_prefix(COLUMN) {
                    let digits = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
                    expected.col =
                        Some(after[..digits].parse().map_err(|_| Malformed::Column { line: number })?);
                    rest = &after[digits..];
                } else if let Some(after) = rest.strip_prefix(FRAGMENT) {
                    let (fragment, after) =
                        after.split_once(FRAGMENT).ok_or(Malformed::Fragment { line: number })?;
                    expected.fragments.push(fragment.to_owned());
                    rest = after;
                } else {
                    break;
                }
            }
            found.push(expected);
        }
        Ok(found)
    }
}

/// The difference between expected and reported diagnostics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mismatch {
    pub missing: BTreeSet<At>,
    pub unexpected: BTreeSet<At>,
    /// Annotations whose diagnostic was reported, but at another column or without a fragment.
    pub differing: Vec<(Expected, Vec<Reported>)>,
}

impl Mismatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.unexpected.is_empty() && self.differing.is_empty()
    }

    /// How the diagnostics `reported` differ from the annotations `expected`.
    #[must_use]
    pub fn between(expected: &[Expected], reported: &[Reported]) -> Self {
        let expected_at: BTreeSet<At> = expected.iter().map(|e| e.at.clone()).collect();
        let reported_at: BTreeSet<At> = reported.iter().map(|r| r.at.clone()).collect();
        let differing = expected
            .iter()
            .filter(|e| reported_at.contains(&e.at) && !reported.iter().any(|r| e.matches(r)))
            .map(|e| (e.clone(), reported.iter().filter(|r| r.at == e.at).cloned().collect()))
            .collect();
        Self {
            missing: expected_at.difference(&reported_at).cloned().collect(),
            unexpected: reported_at.difference(&expected_at).cloned().collect(),
            differing,
        }
    }
}

/// The outcome of a snapshot check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Snapshot {
    Matches,
    /// The snapshot was written, because blessing was requested or it did not exist.
    Written,
    Differs {
        expected: String,
        actual: String,
    },
}

/// A snapshot that no longer matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
    pub source: PathBuf,
    pub expected: String,
    pub actual: String,
}

impl Snapshot {
    /// Compares `actual` with the snapshot at `path`, or writes it when blessing.
    ///
    /// # Errors
    /// Returns an error if the snapshot cannot be read or written.
    pub fn check(path: &Path, actual: &str) -> io::Result<Self> {
        let bless = std::env::var_os(BLESS_VAR).is_some();
        match fs::read_to_string(path) {
            Ok(expected) if expected == actual => Ok(Self::Matches),
            Ok(expected) if !bless => Ok(Self::Differs { expected, actual: actual.to_owned() }),
            Ok(_) => Self::write(path, actual),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Self::write(path, actual),
            Err(e) => Err(e),
        }
    }

    /// Snapshots `render(text)` of every file under `dir` with extension `ext`, at the same relative
    /// path under `snapshots` with the extension `kind`.
    ///
    /// # Errors
    /// Returns an error if a source or snapshot cannot be read or written.
    pub fn tree(
        dir: &Path,
        ext: &str,
        snapshots: &Path,
        kind: &str,
        render: impl Fn(&str) -> String,
    ) -> io::Result<Vec<Changed>> {
        let mut changed = Vec::new();
        for source in dir.files_with(ext)? {
            let actual = render(&fs::read_to_string(&source)?);
            let relative = source.strip_prefix(dir).unwrap_or(&source);
            let target = snapshots.join(relative).with_extension(kind);
            if let Self::Differs { expected, actual } = Self::check(&target, &actual)? {
                changed.push(Changed { source, expected, actual });
            }
        }
        Ok(changed)
    }

    fn write(path: &Path, contents: &str) -> io::Result<Self> {
        path.put(contents)?;
        Ok(Self::Written)
    }
}

#[cfg(test)]
mod tests;
