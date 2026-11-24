//! `cargo xtask decisions`: the decisions of `docs/spec/decisions/`, one file each, are read and
//! checked, and their index is written. A number is never reused; code and specs cite it.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use vun_data::Object;
use vun_fs::{Edit, Tree};
use vun_term::{Record, Status as Progress, Style, Text, Verb};

mod entry;
mod index;
#[cfg(test)]
mod tests;

pub(crate) use entry::{Decision, Malformed};

/// Where the files are, relative to the repository root.
pub(crate) const DIR: &str = "docs/spec/decisions";
/// What is written beside them, and what is not a decision.
const INDEX: &str = "README";
const QUESTIONS: &str = "questions";
const FILE_EXTENSION: &str = ".md";

/// Every decision, by number.
pub(crate) struct Decisions(Vec<Decision>);

/// Why the decisions are not read.
#[derive(Debug)]
pub(crate) enum ReadError {
    Io(io::Error),
    Malformed {
        path: PathBuf,
        reason: Malformed,
    },
    /// A file whose name is no number and no reserved name.
    Stray(PathBuf),
    /// A file named for one number holding another.
    Misnumbered {
        path: PathBuf,
        held: u32,
    },
    /// A decision citing one that does not exist.
    Dangling {
        number: u32,
        cited: u32,
    },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => write!(f, "the decisions cannot be read"),
            Self::Malformed { path, .. } => write!(f, "`{}` is not a decision", path.display()),
            Self::Stray(path) => write!(f, "`{}` is named for no decision", path.display()),
            Self::Misnumbered { path, held } => write!(f, "`{}` holds decision {held}", path.display()),
            Self::Dangling { number, cited } => {
                write!(f, "decision {number} cites {cited}, which is not one")
            }
        }
    }
}

impl Error for ReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Malformed { reason, .. } => Some(reason),
            Self::Stray(_) | Self::Misnumbered { .. } | Self::Dangling { .. } => None,
        }
    }
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl Decisions {
    /// The file of decision `number`.
    pub(crate) fn file_name(number: u32) -> String {
        format!("{number:03}{FILE_EXTENSION}")
    }

    pub(crate) fn all(&self) -> &[Decision] {
        &self.0
    }

    /// Every decision under `root`, by number.
    ///
    /// # Errors
    /// Fails when a file cannot be read or is no decision, or a decision cites one that is not.
    pub(crate) fn read(root: &Path) -> Result<Self, ReadError> {
        let mut all = Vec::new();
        for path in root.join(DIR).files_with(FILE_EXTENSION.trim_start_matches('.'))? {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            if stem == INDEX || stem == QUESTIONS {
                continue;
            }
            let Ok(named) = stem.parse::<u32>() else { return Err(ReadError::Stray(path)) };
            let decision = Decision::parse(&std::fs::read_to_string(&path)?)
                .map_err(|reason| ReadError::Malformed { path: path.clone(), reason })?;
            if decision.number != named {
                return Err(ReadError::Misnumbered { path, held: decision.number });
            }
            all.push(decision);
        }
        all.sort_by_key(|d| d.number);
        let held = |number: u32| all.binary_search_by_key(&number, |d| d.number).is_ok();
        for d in &all {
            if let Some(&cited) = d.amended_by.iter().chain(&d.superseded_by).find(|&&n| !held(n)) {
                return Err(ReadError::Dangling { number: d.number, cited });
            }
        }
        Ok(Self(all))
    }

    /// Writes the index under `root` unless it already holds what the decisions give; whether it wrote.
    ///
    /// # Errors
    /// Returns an error if the index cannot be written.
    pub(crate) fn refresh(&self, root: &Path) -> io::Result<bool> {
        let path = root.join(DIR).join(format!("{INDEX}{FILE_EXTENSION}"));
        let index = self.index();
        if std::fs::read_to_string(&path).is_ok_and(|held| held == index) {
            return Ok(false);
        }
        path.put(index).map(|()| true)
    }
}

/// The index written again.
pub(crate) struct Written;

impl Record for Written {
    fn kind(&self) -> &'static str {
        "written"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, format!("{DIR}/{INDEX}{FILE_EXTENSION}"))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("path", format!("{DIR}/{INDEX}{FILE_EXTENSION}"))
    }
}

impl Progress for Written {
    fn verb(&self) -> Verb {
        Verb::good("Wrote")
    }
}
