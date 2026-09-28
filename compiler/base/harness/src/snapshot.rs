//! Snapshots: a text rendered from each source, compared with the one kept beside the tests;
//! written only when [`BLESS_VAR`] asks.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use vun_fs::{Edit, Tree};

use crate::BLESS_VAR;

/// The outcome of a snapshot check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Snapshot {
    Matches,
    /// The snapshot was written, because blessing was requested.
    Written,
    Differs {
        expected: String,
        actual: String,
    },
    /// No snapshot exists yet: blessing writes it.
    Missing {
        actual: String,
    },
}

/// A snapshot that no longer holds: the file it is of, and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
    pub source: PathBuf,
    pub change: Change,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Differs {
        expected: String,
        actual: String,
    },
    Missing {
        actual: String,
    },
    /// A snapshot whose source is gone; `source` is the snapshot, which blessing removes.
    Stale,
}

impl std::fmt::Display for Changed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source = self.source.display();
        match &self.change {
            Change::Differs { expected, actual } => {
                write!(f, "--- {source}: expected\n{expected}\n--- actual\n{actual}")
            }
            Change::Missing { actual } => write!(f, "--- {source}: no snapshot\n--- actual\n{actual}"),
            Change::Stale => write!(f, "--- {source}: a snapshot of no source"),
        }
    }
}

impl Snapshot {
    /// Whether [`BLESS_VAR`] asks for snapshots to be written: set, not empty and not `0`.
    #[must_use]
    pub fn blessing() -> bool {
        std::env::var_os(BLESS_VAR).is_some_and(|v| !v.is_empty() && v != "0")
    }

    /// Compares `actual` with the snapshot at `path`, or writes it when blessing.
    ///
    /// # Errors
    /// Returns an error if the snapshot cannot be read or written.
    pub fn check(path: &Path, actual: &str) -> io::Result<Self> {
        let expected = match fs::read_to_string(path) {
            Ok(expected) => Some(expected),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        match expected {
            Some(expected) if expected == actual => Ok(Self::Matches),
            _ if Self::blessing() => Self::write(path, actual),
            Some(expected) => Ok(Self::Differs { expected, actual: actual.to_owned() }),
            None => Ok(Self::Missing { actual: actual.to_owned() }),
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
        let mut taken = Vec::new();
        for source in dir.files_with(ext)? {
            let actual = render(&fs::read_to_string(&source)?);
            let relative = source.strip_prefix(dir).unwrap_or(&source);
            let target = snapshots.join(relative).with_extension(kind);
            let change = match Self::check(&target, &actual)? {
                Self::Matches | Self::Written => None,
                Self::Differs { expected, actual } => Some(Change::Differs { expected, actual }),
                Self::Missing { actual } => Some(Change::Missing { actual }),
            };
            changed.extend(change.map(|change| Changed { source, change }));
            taken.push(target);
        }
        let written = if snapshots.is_dir() { snapshots.files_with(kind)? } else { Vec::new() };
        for stale in written.into_iter().filter(|s| !taken.contains(s)) {
            if Self::blessing() {
                fs::remove_file(&stale)?;
            } else {
                changed.push(Changed { source: stale, change: Change::Stale });
            }
        }
        Ok(changed)
    }

    fn write(path: &Path, contents: &str) -> io::Result<Self> {
        path.put(contents)?;
        Ok(Self::Written)
    }
}
