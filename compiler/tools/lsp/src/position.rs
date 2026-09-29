//! Positions as the protocol counts them, a line and a character in it, and the byte offsets the
//! compiler counts: each version of a document keeps where its lines start, and a character is a
//! byte, a UTF-16 unit or a scalar value, as the client and the server agree.

use vun_lsp_protocol::base::{Position, PositionEncoding, Range};

/// How the server counts in an encoding, and which it prefers.
pub(crate) trait Counting: Sized {
    /// The encodings the server prefers, most first; the protocol's default otherwise.
    const PREFERRED: [Self; 2];

    /// The preferred encoding among those `offered`.
    fn chosen(offered: &[Self]) -> Self;

    /// The count of `c` in this encoding.
    fn width(self, c: char) -> u32;
}

impl Counting for PositionEncoding {
    const PREFERRED: [Self; 2] = [Self::Utf8, Self::Utf32];

    fn chosen(offered: &[Self]) -> Self {
        Self::PREFERRED.into_iter().find(|e| offered.contains(e)).unwrap_or(Self::DEFAULT)
    }

    fn width(self, c: char) -> u32 {
        let width = match self {
            Self::Utf8 => c.len_utf8(),
            Self::Utf16 => c.len_utf16(),
            Self::Utf32 => 1,
        };
        u32::try_from(width).unwrap_or(u32::MAX)
    }
}

/// Where each line of a text starts.
pub(crate) struct Lines {
    starts: Vec<u32>,
}

impl Lines {
    pub(crate) fn of(text: &str) -> Self {
        let breaks = text.match_indices('\n').filter_map(|(i, _)| u32::try_from(i + 1).ok());
        Self { starts: std::iter::once(0).chain(breaks).collect() }
    }

    /// The position of the byte `offset` of `text`.
    pub(crate) fn position(&self, text: &str, offset: u32, encoding: PositionEncoding) -> Position {
        let line = self.starts.partition_point(|&s| s <= offset).saturating_sub(1);
        let start = self.starts[line];
        let before = text.get(start as usize..offset as usize).unwrap_or_default();
        let character = before.chars().map(|c| encoding.width(c)).sum();
        Position { line: u32::try_from(line).unwrap_or(u32::MAX), character }
    }

    /// The byte offset of `position` in `text`, the end of its line past its last character.
    pub(crate) fn offset(&self, text: &str, position: Position, encoding: PositionEncoding) -> Option<u32> {
        let line = position.line as usize;
        let start = *self.starts.get(line)?;
        let end = self.starts.get(line + 1).copied().unwrap_or(u32::try_from(text.len()).ok()?);
        let mut counted = 0;
        for (i, c) in text.get(start as usize..end as usize)?.char_indices() {
            if counted >= position.character || c == '\n' {
                return Some(start + u32::try_from(i).ok()?);
            }
            counted += encoding.width(c);
        }
        Some(end)
    }

    /// The range of the bytes `lo..hi` of `text`.
    pub(crate) fn range(&self, text: &str, lo: u32, hi: u32, encoding: PositionEncoding) -> Range {
        Range { start: self.position(text, lo, encoding), end: self.position(text, hi, encoding) }
    }

    /// The line holding the byte `offset`.
    pub(crate) fn line(&self, offset: u32) -> u32 {
        let line = self.starts.partition_point(|&s| s <= offset).saturating_sub(1);
        u32::try_from(line).unwrap_or(u32::MAX)
    }
}
