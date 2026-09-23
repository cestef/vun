//! Source files, file ids and byte spans.
//!
//! A [`Span`] is a half-open byte range inside one source file. Spans never cross files; anything
//! that relates several locations does so through origin chains (`overt-origin`).

use std::hash::{Hash, Hasher};

/// The extension of Overt source files.
pub const EXTENSION: &str = "ov";

/// A source file within one compilation session. It hashes by its key, which names the file the
/// same way in every process (its package and its path inside it), so that hashes of spans and of
/// anything holding them do not depend on the order files were read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileId {
    index: u32,
    key: u64,
}

impl Hash for FileId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.key);
    }
}

/// A half-open byte range `lo..hi`, relative to some text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ByteRange {
    pub lo: u32,
    pub hi: u32,
}

impl ByteRange {
    /// # Panics
    /// Panics if `lo > hi`.
    #[must_use]
    pub const fn new(lo: u32, hi: u32) -> Self {
        assert!(lo <= hi, "range starts after it ends");
        Self { lo, hi }
    }

    #[must_use]
    pub const fn len(self) -> u32 {
        self.hi - self.lo
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.lo == self.hi
    }

    /// The smallest range covering both.
    #[must_use]
    pub fn cover(self, other: Self) -> Self {
        Self { lo: self.lo.min(other.lo), hi: self.hi.max(other.hi) }
    }

    /// The text this range selects.
    #[must_use]
    pub fn of(self, text: &str) -> &str {
        &text[self.lo as usize..self.hi as usize]
    }
}

/// A byte range in one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: FileId,
    pub range: ByteRange,
}

impl Span {
    /// # Panics
    /// Panics if `lo > hi`.
    #[must_use]
    pub const fn new(file: FileId, lo: u32, hi: u32) -> Self {
        Self { file, range: ByteRange::new(lo, hi) }
    }

    /// The smallest span covering both.
    ///
    /// # Panics
    /// Panics if the spans are in different files.
    #[must_use]
    pub fn cover(self, other: Self) -> Self {
        assert_eq!(self.file, other.file, "cannot cover spans from different files");
        Self { file: self.file, range: self.range.cover(other.range) }
    }
}

/// A one-based line and column, the column counted in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    pub line: u32,
    pub col: u32,
}

/// A source file and the byte offset of each of its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// As shown to users.
    pub path: String,
    pub text: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    /// # Panics
    /// Panics if the text is 4 GiB or larger.
    #[must_use]
    pub fn new(path: String, text: String) -> Self {
        let newlines = text.match_indices('\n').map(|(i, _)| i + 1);
        let line_starts = std::iter::once(0)
            .chain(newlines)
            .map(|i| u32::try_from(i).expect("source files are smaller than 4 GiB"))
            .collect();
        Self { path, text, line_starts }
    }

    #[must_use]
    #[expect(clippy::missing_panics_doc, reason = "line indices fit u32 because offsets do")]
    pub fn line_col(&self, offset: u32) -> LineCol {
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        LineCol {
            line: u32::try_from(line + 1).expect("line numbers fit u32"),
            col: offset - self.line_starts[line] + 1,
        }
    }

    /// Each line's text and range, without its line break.
    ///
    /// # Panics
    /// Panics if the text is 4 GiB or larger, which [`SourceFile::new`] rejects.
    pub fn lines(&self) -> impl Iterator<Item = (&str, ByteRange)> {
        let ends = self.line_starts.iter().skip(1).copied();
        let len = u32::try_from(self.text.len()).expect("source files are smaller than 4 GiB");
        let ranges = self.line_starts.iter().zip(ends.chain([len])).map(|(&lo, hi)| ByteRange::new(lo, hi));
        ranges.filter(|r| !r.is_empty()).map(|r| {
            let line = r.of(&self.text).trim_end_matches(['\n', '\r']);
            let hi = r.lo + u32::try_from(line.len()).expect("lines are shorter than their file");
            (line, ByteRange::new(r.lo, hi))
        })
    }
}

/// Read access to source files.
pub trait SourceMap {
    fn file(&self, id: FileId) -> &SourceFile;

    /// Where `span` starts, as shown to users.
    fn locate(&self, span: Span) -> Location<'_> {
        let file = self.file(span.file);
        Location { path: &file.path, at: file.line_col(span.range.lo) }
    }
}

/// A position as shown to users; displays as `path:line:col`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location<'a> {
    pub path: &'a str,
    pub at: LineCol,
}

impl std::fmt::Display for Location<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.path, self.at.line, self.at.col)
    }
}

/// An in-memory set of source files, appended to through a shared reference, so that every
/// query of a compilation adds to one table.
#[derive(Default)]
pub struct SourceFiles {
    files: boxcar::Vec<SourceFile>,
    /// Each file by its key and the hash of its text, so that adding it again returns it.
    added: std::sync::RwLock<std::collections::HashMap<(u64, overt_hash::Hash), FileId>>,
}

impl SourceFiles {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file keyed by its path.
    pub fn add(&self, path: impl Into<String>, text: impl Into<String>) -> FileId {
        let path = path.into();
        let key = path.clone();
        self.add_as(&key, path, text)
    }

    /// Adds a file under `key`, which names it the same way in every process, shown as `path`;
    /// the same key with the same text is the file already added.
    ///
    /// # Panics
    /// Panics past `u32::MAX` files, or if a thread panicked while adding.
    pub fn add_as(&self, key: &str, path: impl Into<String>, text: impl Into<String>) -> FileId {
        let key = overt_hash::Hash::of(key.as_bytes()).short();
        let text = text.into();
        let identity = (key, overt_hash::Hash::of(text.as_bytes()));
        if let Some(&file) = self.added.read().expect("source files").get(&identity) {
            return file;
        }
        let mut added = self.added.write().expect("source files");
        *added.entry(identity).or_insert_with(|| {
            let index = self.files.push(SourceFile::new(path.into(), text));
            FileId { index: u32::try_from(index).expect("fewer than 2^32 files"), key }
        })
    }
}

impl SourceMap for SourceFiles {
    fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id.index as usize]
    }
}

#[cfg(test)]
mod tests;
