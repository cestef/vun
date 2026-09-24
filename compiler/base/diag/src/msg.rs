//! Structured messages.

use std::borrow::Cow;
use std::fmt;

/// One part of a message. Structured parts let renderers style them and machine formats keep
/// them apart.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MsgPart {
    Text(Cow<'static, str>),
    Code(String),
    Type(String),
    Num(i128),
}

/// A message, built from typed parts rather than formatted text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Msg(pub Vec<MsgPart>);

impl Msg {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn text(self, text: impl Into<Cow<'static, str>>) -> Self {
        self.part(MsgPart::Text(text.into()))
    }

    #[must_use]
    pub fn code(self, code: impl Into<String>) -> Self {
        self.part(MsgPart::Code(code.into()))
    }

    #[must_use]
    pub fn ty(self, ty: impl Into<String>) -> Self {
        self.part(MsgPart::Type(ty.into()))
    }

    #[must_use]
    pub fn num(self, n: impl Into<i128>) -> Self {
        self.part(MsgPart::Num(n.into()))
    }

    /// The help offering `near` for a misspelled name.
    #[must_use]
    pub fn did_you_mean(near: impl Into<String>) -> Self {
        Self::new().text("did you mean ").code(near).text("?")
    }

    /// Items rendered as code, separated by commas.
    #[must_use]
    pub fn code_list<S: Into<String>>(mut self, items: impl IntoIterator<Item = S>) -> Self {
        for (i, item) in items.into_iter().enumerate() {
            if i > 0 {
                self = self.text(", ");
            }
            self = self.code(item);
        }
        self
    }

    /// A path of names rendered as code, as written: `std.mem`.
    #[must_use]
    pub fn path<S: AsRef<str>>(self, names: impl IntoIterator<Item = S>) -> Self {
        let names: Vec<S> = names.into_iter().collect();
        self.code(names.iter().map(AsRef::as_ref).collect::<Vec<_>>().join("."))
    }

    /// The parts of `other`, after these.
    #[must_use]
    pub fn append(mut self, other: Self) -> Self {
        self.0.extend(other.0);
        self
    }

    fn part(mut self, part: MsgPart) -> Self {
        self.0.push(part);
        self
    }
}

impl<T: Into<Cow<'static, str>>> From<T> for Msg {
    fn from(text: T) -> Self {
        Self(vec![MsgPart::Text(text.into())])
    }
}

/// Plain text, with code and types in backticks.
impl fmt::Display for Msg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for part in &self.0 {
            match part {
                MsgPart::Text(t) => f.write_str(t)?,
                MsgPart::Code(c) | MsgPart::Type(c) => write!(f, "`{c}`")?,
                MsgPart::Num(n) => write!(f, "{n}")?,
            }
        }
        Ok(())
    }
}
