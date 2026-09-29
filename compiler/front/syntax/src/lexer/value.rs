//! What string and character tokens denote: a string its bytes, each escape one unit (`\xHH` one
//! byte, `\u{..}` a character's UTF-8), and a character the one unit it holds.

use std::borrow::Cow;
use std::ops::Range;

use vun_grammar::TokenKind;

use super::{BYTE_ESCAPE_DIGITS, ESCAPE, RAW_STRING_LINE, SIMPLE_ESCAPES};

/// A piece of a literal's body: a byte written as `\xHH`, or a character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Byte(u8),
    Char(char),
}

/// The tokens writing strings.
const STRINGS: [TokenKind; 3] = [TokenKind::String, TokenKind::RawString, TokenKind::RawStringLine];

/// [`located`] for the text of a string token of any kind, as a literal's source gives it.
#[must_use]
pub fn located_string(text: &str) -> Option<Vec<(u8, Range<usize>)>> {
    STRINGS.iter().find_map(|&kind| located(kind, text))
}

/// Where the body of a string token starts in its text, and the body.
fn body(kind: TokenKind, text: &str) -> Option<(usize, &str)> {
    let quoted =
        |open: &'static str, close: char| Some((open.len(), text.strip_prefix(open)?.strip_suffix(close)?));
    match kind {
        TokenKind::String => quoted("\"", '"'),
        TokenKind::RawString => quoted("\\\"", '"'),
        TokenKind::RawStringLine => Some((RAW_STRING_LINE.len(), text.strip_prefix(RAW_STRING_LINE)?)),
        _ => None,
    }
}

/// The bytes a string token denotes.
#[must_use]
pub fn bytes(kind: TokenKind, text: &str) -> Option<Cow<'_, [u8]>> {
    let (_, body) = body(kind, text)?;
    if kind != TokenKind::String || !body.contains(ESCAPE) {
        return Some(Cow::Borrowed(body.as_bytes()));
    }
    Some(Cow::Owned(located(kind, text)?.into_iter().map(|(b, _)| b).collect()))
}

/// The bytes a string token denotes, each with the range of `text` writing it: an escape's
/// whole range for each byte it gives, so that a part of the value is found in the source.
#[must_use]
pub fn located(kind: TokenKind, text: &str) -> Option<Vec<(u8, Range<usize>)>> {
    let (open, body) = body(kind, text)?;
    let shifted = |r: Range<usize>| r.start + open..r.end + open;
    if kind != TokenKind::String {
        return Some(body.bytes().enumerate().map(|(i, b)| (b, shifted(i..i + 1))).collect());
    }
    let mut out = Vec::with_capacity(body.len());
    for unit in units(body) {
        let (unit, at) = unit?;
        match unit {
            Unit::Byte(b) => out.push((b, shifted(at))),
            Unit::Char(c) => out.extend(c.encode_utf8(&mut [0; 4]).bytes().map(|b| (b, shifted(at.clone())))),
        }
    }
    Some(out)
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
    let (unit, _) = body.next()??;
    body.next().is_none().then_some(match unit {
        Unit::Byte(b) => char::from(b),
        Unit::Char(c) => c,
    })
}

/// The units of an escaped body, each with the range of the body writing it; `None` for a
/// malformed escape.
fn units(body: &str) -> impl Iterator<Item = Option<(Unit, Range<usize>)>> + '_ {
    let mut chars = body.chars();
    let at = |rest: &str| body.len() - rest.len();
    std::iter::from_fn(move || {
        let start = at(chars.as_str());
        let c = chars.next()?;
        let unit = if c == ESCAPE {
            match chars.next() {
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
            }
        } else {
            Some(Unit::Char(c))
        };
        Some(unit.map(|u| (u, start..at(chars.as_str()))))
    })
}
