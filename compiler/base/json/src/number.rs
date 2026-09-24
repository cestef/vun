//! Numbers as their text: the grammar says what a number is, not how wide, so a reader keeps the
//! digits and each use converts them to the type it needs, exactly or not at all.

use std::borrow::Cow;
use std::fmt;

/// A JSON number, its text valid by the grammar of RFC 8259, section 6.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Number<'a>(Cow<'a, str>);

impl<'a> Number<'a> {
    /// `text` checked against the grammar: `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`.
    #[must_use]
    pub fn parse(text: &'a str) -> Option<Self> {
        (scan(text.as_bytes()) == Some(text.len())).then_some(Self(Cow::Borrowed(text)))
    }

    /// The number of a finite float, written as the shortest text that reads back as it; `None`
    /// for infinities and NaN, which JSON cannot write.
    #[must_use]
    pub fn from_f64(x: f64) -> Option<Self> {
        x.is_finite().then(|| Self(Cow::Owned(format!("{x:?}"))))
    }

    /// The text, as read or written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the text is an integer: no fraction and no exponent.
    #[must_use]
    pub fn is_integer(&self) -> bool {
        !self.0.bytes().any(|b| matches!(b, b'.' | b'e' | b'E'))
    }

    /// The value, when it is an integer that fits.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        self.0.parse().ok()
    }

    /// The value, when it is an integer that fits.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.0.parse().ok()
    }

    /// The nearest float, rounded as IEEE 754 prescribes; infinite past its range.
    #[must_use]
    pub fn as_f64(&self) -> f64 {
        // The grammar is a subset of what `f64::from_str` reads.
        self.0.parse().unwrap_or(f64::NAN)
    }

    #[must_use]
    pub fn into_owned(self) -> Number<'static> {
        Number(Cow::Owned(self.0.into_owned()))
    }

    pub(crate) fn borrowed(text: &'a str) -> Self {
        Self(Cow::Borrowed(text))
    }
}

/// How many bytes at the start of `bytes` a number takes; `None` when they begin no number.
pub(crate) fn scan(bytes: &[u8]) -> Option<usize> {
    let digits = |from: usize| bytes[from..].iter().take_while(|b| b.is_ascii_digit()).count();
    let mut at = usize::from(bytes.first() == Some(&b'-'));
    match bytes.get(at)? {
        b'0' => at += 1,
        b'1'..=b'9' => at += digits(at),
        _ => return None,
    }
    if bytes.get(at) == Some(&b'.') {
        let n = digits(at + 1);
        if n == 0 {
            return None;
        }
        at += 1 + n;
    }
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let n = digits(at);
        if n == 0 {
            return None;
        }
        at += n;
    }
    Some(at)
}

macro_rules! from_integer {
    ($($t:ty),*) => {$(
        impl From<$t> for Number<'_> {
            fn from(n: $t) -> Self {
                Self(Cow::Owned(n.to_string()))
            }
        }
    )*};
}
from_integer!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl fmt::Display for Number<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
