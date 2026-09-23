//! Source files, file ids and byte spans.
//!
//! A [`Span`] is a half-open byte range inside one source file. Spans never cross files; anything
//! that relates several locations does so through origin chains (`overt-origin`).

use overt_index::{IndexVec, index};

index! {
    /// A source file within one compilation session.
    pub struct FileId;
}

/// A half-open byte range `lo..hi` in one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: FileId,
    pub lo: u32,
    pub hi: u32,
}

impl Span {
    /// # Panics
    /// Panics if `lo > hi`.
    #[must_use]
    pub const fn new(file: FileId, lo: u32, hi: u32) -> Self {
        assert!(lo <= hi, "span starts after it ends");
        Self { file, lo, hi }
    }

    #[must_use]
    pub const fn len(self) -> u32 {
        self.hi - self.lo
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.lo == self.hi
    }

    /// The smallest span covering both.
    ///
    /// # Panics
    /// Panics if the spans are in different files.
    #[must_use]
    pub fn cover(self, other: Self) -> Self {
        assert_eq!(self.file, other.file, "cannot cover spans from different files");
        Self { file: self.file, lo: self.lo.min(other.lo), hi: self.hi.max(other.hi) }
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
}

/// Read access to source files.
pub trait SourceMap {
    fn file(&self, id: FileId) -> &SourceFile;
}

/// An in-memory set of source files.
#[derive(Default)]
pub struct SourceFiles {
    files: IndexVec<FileId, SourceFile>,
}

impl SourceFiles {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, path: impl Into<String>, text: impl Into<String>) -> FileId {
        self.files.push(SourceFile::new(path.into(), text.into()))
    }
}

impl SourceMap for SourceFiles {
    fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_counts_from_one() {
        let file = SourceFile::new("a.ov".into(), "x := 1;\ny := 2;\n".into());
        assert_eq!(file.line_col(0), LineCol { line: 1, col: 1 });
        assert_eq!(file.line_col(8), LineCol { line: 2, col: 1 });
        assert_eq!(file.line_col(10), LineCol { line: 2, col: 3 });
    }

    #[test]
    fn cover_joins_ranges() {
        let mut files = SourceFiles::new();
        let f = files.add("a.ov", "");
        assert_eq!(Span::new(f, 4, 6).cover(Span::new(f, 1, 5)), Span::new(f, 1, 6));
    }
}
