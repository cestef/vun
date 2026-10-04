//! A place in a file as people write it: a one-based line and a column of characters.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use crate::SourceFile;

/// The separator of a line and a column.
const SEPARATOR: char = ':';

/// A one-based line and a one-based column counted in characters, written `line:column`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub line: u32,
    pub column: u32,
}

/// Why a text is no [`Place`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoPlace;

impl fmt::Display for NoPlace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a place is a line and a column counted from 1, written line{SEPARATOR}column")
    }
}

impl Error for NoPlace {}

impl FromStr for Place {
    type Err = NoPlace;

    fn from_str(text: &str) -> Result<Self, NoPlace> {
        let (line, column) = text.split_once(SEPARATOR).ok_or(NoPlace)?;
        let number = |n: &str| n.parse::<u32>().ok().filter(|&n| n > 0).ok_or(NoPlace);
        Ok(Self { line: number(line)?, column: number(column)? })
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{SEPARATOR}{}", self.line, self.column)
    }
}

impl SourceFile {
    /// The byte offset of `place`, the end of its line for a column past it; none for a line the
    /// file lacks.
    #[must_use]
    pub fn offset(&self, place: Place) -> Option<u32> {
        let start = *self.line_starts.get(usize::try_from(place.line).ok()?.checked_sub(1)?)?;
        let line = self.text.get(start as usize..)?.split('\n').next()?;
        let before: usize =
            line.chars().take(usize::try_from(place.column).ok()?.checked_sub(1)?).map(char::len_utf8).sum();
        Some(start + u32::try_from(before).ok()?)
    }
}
