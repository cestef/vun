//! Strings (RFC 8259, section 7): characters up to the closing quote, control characters escaped,
//! `\u` escapes of UTF-16 code units, surrogate pairs combined. A string without escapes is a
//! slice of the input.

use std::borrow::Cow;

use crate::ErrorKind;

/// The string whose opening quote is at `start - 1` in `text`, and the offset past its closing
/// quote; or what is wrong, and where.
pub(crate) fn read(text: &str, start: usize) -> Result<(Cow<'_, str>, usize), (ErrorKind, usize)> {
    let bytes = text.as_bytes();
    let special = |from: usize| {
        bytes[from..].iter().position(|&b| b == b'"' || b == b'\\' || b < 0x20).map(|n| from + n)
    };
    let mut at = special(start).ok_or((ErrorKind::UnexpectedEnd, bytes.len()))?;
    if bytes[at] == b'"' {
        return Ok((Cow::Borrowed(&text[start..at]), at + 1));
    }
    let mut owned = String::with_capacity(at - start + 16);
    owned.push_str(&text[start..at]);
    loop {
        match bytes[at] {
            b'"' => return Ok((Cow::Owned(owned), at + 1)),
            b'\\' => at = escape(bytes, at, &mut owned)?,
            _ => return Err((ErrorKind::ControlInString, at)),
        }
        let next = special(at).ok_or((ErrorKind::UnexpectedEnd, bytes.len()))?;
        owned.push_str(&text[at..next]);
        at = next;
    }
}

/// Appends the character the escape at `at` stands for; the offset past it.
fn escape(bytes: &[u8], at: usize, out: &mut String) -> Result<usize, (ErrorKind, usize)> {
    let c = match bytes.get(at + 1).ok_or((ErrorKind::UnexpectedEnd, bytes.len()))? {
        b'"' => '"',
        b'\\' => '\\',
        b'/' => '/',
        b'b' => '\u{8}',
        b'f' => '\u{c}',
        b'n' => '\n',
        b'r' => '\r',
        b't' => '\t',
        b'u' => return unicode(bytes, at, out),
        _ => return Err((ErrorKind::InvalidEscape, at)),
    };
    out.push(c);
    Ok(at + 2)
}

/// `\uXXXX`, or two of them forming a surrogate pair.
fn unicode(bytes: &[u8], at: usize, out: &mut String) -> Result<usize, (ErrorKind, usize)> {
    let first = unit(bytes, at + 2)?;
    let (code, end) = match first {
        0xD800..=0xDBFF => {
            if bytes.get(at + 6..at + 8) != Some(b"\\u") {
                return Err((ErrorKind::LoneSurrogate, at));
            }
            let second = unit(bytes, at + 8)?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err((ErrorKind::LoneSurrogate, at));
            }
            (0x10000 + ((u32::from(first) - 0xD800) << 10) + (u32::from(second) - 0xDC00), at + 12)
        }
        0xDC00..=0xDFFF => return Err((ErrorKind::LoneSurrogate, at)),
        unit => (u32::from(unit), at + 6),
    };
    // Every code point outside the surrogates is a scalar value.
    out.push(char::from_u32(code).ok_or((ErrorKind::LoneSurrogate, at))?);
    Ok(end)
}

/// The UTF-16 code unit of the four hexadecimal digits at `at`.
fn unit(bytes: &[u8], at: usize) -> Result<u16, (ErrorKind, usize)> {
    let digits = bytes.get(at..at + 4).ok_or((ErrorKind::UnexpectedEnd, bytes.len()))?;
    digits.iter().try_fold(0u16, |n, &d| {
        let v = char::from(d).to_digit(16).ok_or((ErrorKind::InvalidEscape, at))?;
        // Four digits never overflow sixteen bits.
        Ok(n << 4 | u16::try_from(v).unwrap_or(0))
    })
}
