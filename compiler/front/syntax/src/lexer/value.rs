//! The text that string and character tokens denote.

use std::borrow::Cow;

use vun_grammar::TokenKind;

use super::{BYTE_ESCAPE_DIGITS, ESCAPE, RAW_STRING_LINE, SIMPLE_ESCAPES};

#[must_use]
pub fn literal(kind: TokenKind, text: &str) -> Option<Cow<'_, str>> {
    let quoted = |open: &str, close: char| text.strip_prefix(open)?.strip_suffix(close);
    match kind {
        TokenKind::String => unescape(quoted("\"", '"')?),
        TokenKind::Char => unescape(quoted("'", '\'')?),
        TokenKind::RawString => Some(Cow::Borrowed(quoted("\\\"", '"')?)),
        TokenKind::RawStringLine => Some(Cow::Borrowed(text.strip_prefix(RAW_STRING_LINE)?)),
        _ => None,
    }
}

pub(super) fn unescape(body: &str) -> Option<Cow<'_, str>> {
    if !body.contains(ESCAPE) {
        return Some(Cow::Borrowed(body));
    }
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != ESCAPE {
            out.push(c);
            continue;
        }
        let value = match chars.next()? {
            'x' => {
                let hex: String = chars.by_ref().take(BYTE_ESCAPE_DIGITS).collect();
                char::from(u8::from_str_radix(&hex, 16).ok()?)
            }
            'u' => {
                let (hex, after) = chars.as_str().strip_prefix('{')?.split_once('}')?;
                chars = after.chars();
                char::from_u32(u32::from_str_radix(hex, 16).ok()?)?
            }
            e => SIMPLE_ESCAPES.iter().find(|&&(c, _)| c == e)?.1,
        };
        out.push(value);
    }
    Some(Cow::Owned(out))
}
