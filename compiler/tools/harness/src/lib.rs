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

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Marker that starts an expectation inside a line comment.
pub const MARKER: &str = "//~";
/// Environment variable that makes snapshot checks overwrite instead of compare.
pub const BLESS_VAR: &str = "OVERT_BLESS";

/// A diagnostic expected or reported on a one-based line.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct At {
    pub line: u32,
    pub id: String,
}

/// Expectations written in `text`.
#[must_use]
pub fn expectations(text: &str) -> BTreeSet<At> {
    text.lines()
        .zip(1u32..)
        .filter_map(|(line, number)| {
            let rest = &line[line.find(MARKER)? + MARKER.len()..];
            let up = rest.chars().take_while(|&c| c == '^').count();
            let id = rest[up..].trim();
            let up = u32::try_from(up).ok()?;
            Some(At { line: number.checked_sub(up)?, id: id.to_owned() })
        })
        .collect()
}

/// The difference between expected and reported diagnostics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mismatch {
    pub missing: BTreeSet<At>,
    pub unexpected: BTreeSet<At>,
}

impl Mismatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.unexpected.is_empty()
    }
}

#[must_use]
pub fn compare(expected: &BTreeSet<At>, reported: &BTreeSet<At>) -> Mismatch {
    Mismatch {
        missing: expected.difference(reported).cloned().collect(),
        unexpected: reported.difference(expected).cloned().collect(),
    }
}

/// Files under `dir` with extension `ext`, recursively, in a stable order.
///
/// # Errors
/// Returns an error if a directory cannot be read.
pub fn discover(dir: &Path, ext: &str) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == ext) {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
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

/// Compares `actual` with the snapshot at `path`, or writes it when blessing.
///
/// # Errors
/// Returns an error if the snapshot cannot be read or written.
pub fn snapshot(path: &Path, actual: &str) -> io::Result<Snapshot> {
    let bless = std::env::var_os(BLESS_VAR).is_some();
    match fs::read_to_string(path) {
        Ok(expected) if expected == actual => Ok(Snapshot::Matches),
        Ok(expected) if !bless => Ok(Snapshot::Differs { expected, actual: actual.to_owned() }),
        Ok(_) => write(path, actual),
        Err(e) if e.kind() == io::ErrorKind::NotFound => write(path, actual),
        Err(e) => Err(e),
    }
}

fn write(path: &Path, contents: &str) -> io::Result<Snapshot> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, contents)?;
    Ok(Snapshot::Written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: u32, id: &str) -> At {
        At { line, id: id.to_owned() }
    }

    #[test]
    fn parses_same_line_and_upward_markers() {
        let text = "x: u8 = 300; //~ int.does_not_fit\ny := x + ;\n//~^ syntax.expected_expression\n";
        let expected = expectations(text);
        assert_eq!(expected, [at(1, "int.does_not_fit"), at(2, "syntax.expected_expression")].into());
    }

    #[test]
    fn compare_reports_both_directions() {
        let expected = [at(1, "a"), at(2, "b")].into();
        let reported = [at(2, "b"), at(3, "c")].into();
        let m = compare(&expected, &reported);
        assert_eq!(m.missing, [at(1, "a")].into());
        assert_eq!(m.unexpected, [at(3, "c")].into());
    }
}
