//! What string and character tokens denote: a string its bytes, each escape one unit (`\xHH` one
//! byte, `\u{..}` a character's UTF-8), and a character the one unit it holds.

use std::borrow::Cow;

use vun_grammar::TokenKind;

use super::{BYTE_ESCAPE_DIGITS, ESCAPE, RAW_STRING_LINE, SIMPLE_ESCAPES};

/// A piece of a literal's body: a byte written as `\xHH`, or a character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Byte(u8),
    Char(char),
}

/// The bytes a string token denotes.
#[must_use]
pub fn bytes(kind: TokenKind, text: &str) -> Option<Cow<'_, [u8]>> {
    let quoted = |open: &str, close: char| text.strip_prefix(open)?.strip_suffix(close);
    match kind {
        TokenKind::String => {
            let body = quoted("\"", '"')?;
            if !body.contains(ESCAPE) {
                return Some(Cow::Borrowed(body.as_bytes()));
            }
            let mut out = Vec::with_capacity(body.len());
            for unit in units(body) {
                match unit? {
                    Unit::Byte(b) => out.push(b),
                    Unit::Char(c) => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
                }
            }
            Some(Cow::Owned(out))
        }
        TokenKind::RawString => Some(Cow::Borrowed(quoted("\\\"", '"')?.as_bytes())),
        TokenKind::RawStringLine => Some(Cow::Borrowed(text.strip_prefix(RAW_STRING_LINE)?.as_bytes())),
        _ => None,
    }
}

/// The text a string token denotes, when its bytes are UTF-8.
#[must_use]
pub fn literal(kind: TokenKind, text: &str) -> Option<Cow<'_, str>> {
    match bytes(kind, text)? {
        Cow::Borrowed(b) => std::str::from_utf8(b).ok().map(Cow::Borrowed),
        Cow::Owned(b) => String::from_utf8(b).ok().map(Cow::Owned),
    }
}

/// The Unicode scalar value a character token denotes: its one character, or the byte of `\xHH`.
#[must_use]
pub fn scalar(text: &str) -> Option<char> {
    let mut body = units(text.strip_prefix('\'')?.strip_suffix('\'')?);
    let unit = body.next()??;
    body.next().is_none().then_some(match unit {
        Unit::Byte(b) => char::from(b),
        Unit::Char(c) => c,
    })
}

/// The units of an escaped body, `None` for a malformed escape.
fn units(body: &str) -> impl Iterator<Item = Option<Unit>> + '_ {
    let mut chars = body.chars();
    std::iter::from_fn(move || {
        let c = chars.next()?;
        if c != ESCAPE {
            return Some(Some(Unit::Char(c)));
        }
        Some(match chars.next() {
            Some('x') => {
                let hex: String = chars.by_ref().take(BYTE_ESCAPE_DIGITS).collect();
                u8::from_str_radix(&hex, 16).ok().map(Unit::Byte)
            }
            Some('u') => (|| {
                let (hex, after) = chars.as_str().strip_prefix('{')?.split_once('}')?;
                chars = after.chars();
                char::from_u32(u32::from_str_radix(hex, 16).ok()?).map(Unit::Char)
            })(),
            Some(e) => SIMPLE_ESCAPES.iter().find(|&&(c, _)| c == e).map(|&(_, c)| Unit::Char(c)),
            None => None,
        })
    })
}
