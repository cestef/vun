//! Positions as the protocol counts them, a line and a character in it, and the byte offsets the
//! compiler counts: each version of a document keeps where its lines start, and a character is a
//! byte or a UTF-16 unit, as the client and the server agree.

use vun_json::{Object, Value};

/// How characters of a line are counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Encoding {
    Utf8,
    Utf16,
}

impl Encoding {
    /// Each encoding and the name the protocol gives it, the preferred first.
    const NAMES: [(Self, &'static str); 2] = [(Self::Utf8, "utf-8"), (Self::Utf16, "utf-16")];

    /// The encoding of positions when the client names none it prefers.
    pub(crate) const DEFAULT: Self = Self::Utf16;

    pub(crate) fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(e, _)| *e == self).map_or("utf-16", |(_, n)| n)
    }

    /// The preferred encoding among those the client offers, `names`.
    pub(crate) fn chosen<'v>(names: impl Iterator<Item = &'v Value<'v>>) -> Self {
        let offered: Vec<_> = names.filter_map(Value::as_str).collect();
        Self::NAMES.iter().find(|(_, n)| offered.contains(n)).map_or(Self::DEFAULT, |&(e, _)| e)
    }

    /// The count of `c` in this encoding.
    fn width(self, c: char) -> u32 {
        let width = match self {
            Self::Utf8 => c.len_utf8(),
            Self::Utf16 => c.len_utf16(),
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
    pub(crate) fn position(&self, text: &str, offset: u32, encoding: Encoding) -> Value<'static> {
        let line = self.starts.partition_point(|&s| s <= offset).saturating_sub(1);
        let start = self.starts[line];
        let before = text.get(start as usize..offset as usize).unwrap_or_default();
        let character: u32 = before.chars().map(|c| encoding.width(c)).sum();
        Value::Object(Object::new().with("line", line).with("character", character))
    }

    /// The byte offset of the protocol's `position` in `text`, the end of its line past its last
    /// character.
    pub(crate) fn offset(&self, text: &str, position: &Value<'_>, encoding: Encoding) -> Option<u32> {
        let line = usize::try_from(position.get("line")?.as_u64()?).ok()?;
        let character = u32::try_from(position.get("character")?.as_u64()?).ok()?;
        let start = *self.starts.get(line)?;
        let end = self.starts.get(line + 1).copied().unwrap_or(u32::try_from(text.len()).ok()?);
        let mut counted = 0;
        for (i, c) in text.get(start as usize..end as usize)?.char_indices() {
            if counted >= character || c == '\n' {
                return Some(start + u32::try_from(i).ok()?);
            }
            counted += encoding.width(c);
        }
        Some(end)
    }

    /// The protocol's range of the bytes `lo..hi` of `text`.
    pub(crate) fn range(&self, text: &str, lo: u32, hi: u32, encoding: Encoding) -> Value<'static> {
        let start = self.position(text, lo, encoding);
        Value::Object(Object::new().with("start", start).with("end", self.position(text, hi, encoding)))
    }
}
