//! Percent-encoding (RFC 3986, section 2.1): a byte a component does not allow is written `%XX`,
//! in uppercase hexadecimal. Components are kept as written, and decoded on request.

use std::borrow::Cow;
use std::fmt;
use std::str::Utf8Error;

use crate::chars::Set;
use crate::error::{Invalid, Why};

/// What starts an escape.
const PERCENT: u8 = b'%';
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Text as a URI writes it, its escapes undecoded: a component, or a part of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Encoded<'a>(pub(crate) &'a str);

impl<'a> Encoded<'a> {
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    /// The bytes it stands for, borrowed when it holds no escape.
    #[must_use]
    pub fn decode(self) -> Cow<'a, [u8]> {
        let bytes = self.0.as_bytes();
        if !bytes.contains(&PERCENT) {
            return Cow::Borrowed(bytes);
        }
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if let Some(b) = Escape::at(bytes, i) {
                out.push(b);
                i += Escape::LEN;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        Cow::Owned(out)
    }

    /// The text it stands for.
    ///
    /// # Errors
    /// The bytes it stands for are not UTF-8.
    pub fn decode_utf8(self) -> Result<Cow<'a, str>, Utf8Error> {
        match self.decode() {
            Cow::Borrowed(bytes) => str::from_utf8(bytes).map(Cow::Borrowed),
            Cow::Owned(bytes) => String::from_utf8(bytes).map(Cow::Owned).map_err(|e| e.utf8_error()),
        }
    }
}

impl Encoded<'_> {
    /// The text with its letters in lowercase, but for the digits of its escapes.
    #[must_use]
    pub fn to_ascii_lowercase(self) -> String {
        let bytes = self.0.as_bytes();
        let mut out = String::with_capacity(bytes.len());
        let mut escape = 0;
        for &b in bytes {
            let kept = if escape > 0 { b } else { b.to_ascii_lowercase() };
            escape = if b == PERCENT { Escape::LEN - 1 } else { escape.saturating_sub(1) };
            out.push(char::from(kept));
        }
        out
    }
}

impl fmt::Display for Encoded<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// One escape, `%` and two hexadecimal digits.
struct Escape;

impl Escape {
    const LEN: usize = 3;

    /// The byte escaped at `i` of `bytes`, if an escape is there.
    fn at(bytes: &[u8], i: usize) -> Option<u8> {
        let [PERCENT, hi, lo] = *bytes.get(i..i + Self::LEN)? else { return None };
        let digit = |d: u8| char::from(d).to_digit(16);
        u8::try_from(digit(hi)? * 16 + digit(lo)?).ok()
    }

    fn write(b: u8, out: &mut String) {
        out.push(char::from(PERCENT));
        out.push(char::from(HEX[usize::from(b >> 4)]));
        out.push(char::from(HEX[usize::from(b & 0xf)]));
    }
}

impl Set {
    /// `bytes` written in this set: each byte it does not allow escaped.
    #[must_use]
    pub fn encode(self, bytes: &[u8]) -> Cow<'_, str> {
        match str::from_utf8(bytes) {
            Ok(text) if bytes.iter().all(|&b| self.allows(b)) => Cow::Borrowed(text),
            _ => {
                let mut out = String::with_capacity(bytes.len());
                self.encode_into(bytes, &mut out);
                Cow::Owned(out)
            }
        }
    }

    /// Appends `bytes` written in this set to `out`.
    pub fn encode_into(self, bytes: &[u8], out: &mut String) {
        for &b in bytes {
            if self.allows(b) {
                out.push(char::from(b));
            } else {
                Escape::write(b, out);
            }
        }
    }

    /// Checks that `text` is written in this set: each byte allowed, or part of an escape.
    ///
    /// # Errors
    /// The first byte that is neither, and whether it is a malformed escape.
    pub fn check(self, text: &str) -> Result<(), Invalid> {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == PERCENT && self.escapes() {
                if Escape::at(bytes, i).is_none() {
                    return Err(Invalid::new(Why::Escape, i));
                }
                i += Escape::LEN;
            } else if self.allows(bytes[i]) {
                i += 1;
            } else {
                return Err(Invalid::new(Why::Char(self), i));
            }
        }
        Ok(())
    }

    /// Appends `text`, written in this set, to `out` in its normal form (RFC 3986, section
    /// 6.2.2.2): escapes in uppercase, and those of unreserved bytes decoded.
    pub fn normalize_into(self, text: &str, out: &mut String) {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match Escape::at(bytes, i) {
                Some(b) if Set::Unreserved.allows(b) => out.push(char::from(b)),
                Some(b) => Escape::write(b, out),
                None => {
                    // Not an escape: a run of bytes kept as they are, up to the next `%`.
                    let end =
                        bytes[i + 1..].iter().position(|&b| b == PERCENT).map_or(bytes.len(), |p| i + 1 + p);
                    out.push_str(&text[i..end]);
                    i = end;
                    continue;
                }
            }
            i += Escape::LEN;
        }
    }
}
