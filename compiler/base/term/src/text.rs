//! Styled text: pieces of text, each with the style of what it is. Shown plain through
//! `Display`, styled through [`Out`](crate::Out).

use std::borrow::Cow;
use std::fmt;

use crate::Style;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text(Vec<Piece>);

#[derive(Clone, Debug, PartialEq, Eq, vun_data::ToData)]
pub struct Piece {
    pub style: Option<Style>,
    pub text: Cow<'static, str>,
}

impl Text {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `text` styled `style`, alone.
    #[must_use]
    pub fn of(style: Style, text: impl Into<Cow<'static, str>>) -> Self {
        Self::new().styled(style, text)
    }

    #[must_use]
    pub fn plain(self, text: impl Into<Cow<'static, str>>) -> Self {
        self.piece(None, text.into())
    }

    #[must_use]
    pub fn styled(self, style: Style, text: impl Into<Cow<'static, str>>) -> Self {
        self.piece(Some(style), text.into())
    }

    /// `text` styled `style` when there is a style.
    #[must_use]
    pub fn maybe(self, style: Option<Style>, text: impl Into<Cow<'static, str>>) -> Self {
        self.piece(style, text.into())
    }

    #[must_use]
    pub fn newline(self) -> Self {
        self.plain("\n")
    }

    /// The pieces of `other`, after these.
    #[must_use]
    pub fn append(mut self, other: Self) -> Self {
        self.0.extend(other.0);
        self
    }

    /// `items`, separated by `separator`.
    #[must_use]
    pub fn join(items: impl IntoIterator<Item = Self>, separator: &'static str) -> Self {
        items.into_iter().enumerate().fold(Self::new(), |text, (i, item)| {
            if i == 0 { text.append(item) } else { text.plain(separator).append(item) }
        })
    }

    /// `n` spaces.
    #[must_use]
    pub fn pad(self, n: usize) -> Self {
        if n == 0 { self } else { self.plain(" ".repeat(n)) }
    }

    /// Columns taken on a terminal, one per character.
    #[must_use]
    pub fn width(&self) -> usize {
        self.0.iter().map(|p| p.text.chars().count()).sum()
    }

    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.0
    }

    fn piece(mut self, style: Option<Style>, text: Cow<'static, str>) -> Self {
        if !text.is_empty() {
            self.0.push(Piece { style, text });
        }
        self
    }
}

impl From<&'static str> for Text {
    fn from(text: &'static str) -> Self {
        Self::new().plain(text)
    }
}

impl From<String> for Text {
    fn from(text: String) -> Self {
        Self::new().plain(text)
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|p| f.write_str(&p.text))
    }
}
