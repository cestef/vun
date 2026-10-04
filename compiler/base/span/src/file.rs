//! A source file and where its lines start.

use crate::ByteRange;

/// A one-based line and column, the column counted both in bytes and in characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    pub line: u32,
    pub col: u32,
    pub chars: u32,
}

/// A source file and the byte offset of each of its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// As shown to users.
    pub path: String,
    pub text: String,
    pub(crate) line_starts: Vec<u32>,
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
        let start = self.line_starts[line];
        let before = ByteRange::new(start, offset).of(&self.text);
        LineCol {
            line: u32::try_from(line + 1).expect("line numbers fit u32"),
            col: offset - start + 1,
            chars: u32::try_from(before.chars().count() + 1).expect("columns fit u32 because offsets do"),
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
