//! The reader: iterative, with an explicit stack of the arrays and objects open. Nesting is
//! bounded by [`Parser::max_depth`], which keeps what later recurses over the value, such as
//! dropping it, within the host stack.

use std::borrow::Cow;
use std::fmt;

use crate::number::{Number, scan};
use crate::string;
use crate::{Object, Value};

/// Reads `text` as one JSON value, with the default limits.
///
/// # Errors
/// Returns what breaks the grammar first, and where.
pub fn parse(text: &str) -> Result<Value<'_>, Error> {
    Parser::default().parse(text)
}

/// A reader's limits.
#[derive(Clone, Copy, Debug)]
pub struct Parser {
    /// How many arrays and objects may be open at once. The default, 512, keeps dropping,
    /// comparing and writing the deepest value within a 2 MiB thread stack in a debug build.
    pub max_depth: usize,
}

impl Default for Parser {
    fn default() -> Self {
        Self { max_depth: 512 }
    }
}

/// Why a text is not JSON, at the byte offset where the reader found out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    UnexpectedEnd,
    /// A byte that cannot start or continue the value here.
    Unexpected,
    InvalidNumber,
    InvalidEscape,
    /// A `\u` escape of half a surrogate pair, which is no character.
    LoneSurrogate,
    /// A control character written raw inside a string.
    ControlInString,
    /// Something after the value.
    Trailing,
    TooDeep,
}

impl Error {
    /// The line and column of the offset in `text`, both from 1, the column counting characters.
    #[must_use]
    pub fn line_col(&self, text: &str) -> (usize, usize) {
        let before = &text[..self.offset.min(text.len())];
        let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        (line, column)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.kind {
            ErrorKind::UnexpectedEnd => "the text ends inside a value",
            ErrorKind::Unexpected => "unexpected character",
            ErrorKind::InvalidNumber => "malformed number",
            ErrorKind::InvalidEscape => "malformed escape",
            ErrorKind::LoneSurrogate => "an escape of half a surrogate pair",
            ErrorKind::ControlInString => "a control character inside a string",
            ErrorKind::Trailing => "text after the value",
            ErrorKind::TooDeep => "arrays and objects nested too deep",
        };
        write!(f, "{what} at byte {}", self.offset)
    }
}

impl std::error::Error for Error {}

/// An array or object open, with what it holds so far.
enum Open<'a> {
    Array(Vec<Value<'a>>),
    /// The members so far, and the name of the one whose value is being read.
    Object(Vec<(Cow<'a, str>, Value<'a>)>, Cow<'a, str>),
}

struct Reader<'a> {
    text: &'a str,
    bytes: &'a [u8],
    at: usize,
}

impl Parser {
    /// Reads `text` as one JSON value.
    ///
    /// # Errors
    /// Returns what breaks the grammar first, and where.
    pub fn parse<'a>(&self, text: &'a str) -> Result<Value<'a>, Error> {
        let start = if text.starts_with('\u{feff}') { '\u{feff}'.len_utf8() } else { 0 };
        let mut r = Reader { text, bytes: text.as_bytes(), at: start };
        let mut stack: Vec<Open<'a>> = Vec::new();
        'value: loop {
            r.space();
            let opening = matches!(r.peek()?, b'[' | b'{');
            if opening && stack.len() >= self.max_depth {
                return Err(r.error(ErrorKind::TooDeep));
            }
            let mut value = match r.peek()? {
                b'[' => {
                    r.at += 1;
                    if r.close(b']') {
                        Value::Array(Vec::new())
                    } else {
                        stack.push(Open::Array(Vec::new()));
                        continue 'value;
                    }
                }
                b'{' => {
                    r.at += 1;
                    if r.close(b'}') {
                        Value::Object(Object::new())
                    } else {
                        let name = r.name()?;
                        stack.push(Open::Object(Vec::new(), name));
                        continue 'value;
                    }
                }
                _ => r.scalar()?,
            };
            // Put the value in what is open, closing each array or object it completes.
            loop {
                let Some(open) = stack.last_mut() else {
                    r.space();
                    return if r.at == r.bytes.len() { Ok(value) } else { Err(r.error(ErrorKind::Trailing)) };
                };
                match open {
                    Open::Array(items) => items.push(value),
                    Open::Object(members, name) => members.push((std::mem::take(name), value)),
                }
                r.space();
                match (r.peek()?, &mut *open) {
                    (b',', Open::Array(_)) => {
                        r.at += 1;
                        continue 'value;
                    }
                    (b',', Open::Object(_, name)) => {
                        r.at += 1;
                        r.space();
                        *name = r.name()?;
                        continue 'value;
                    }
                    (b']', Open::Array(_)) | (b'}', Open::Object(..)) => r.at += 1,
                    _ => return Err(r.error(ErrorKind::Unexpected)),
                }
                value = match stack.pop() {
                    Some(Open::Array(items)) => Value::Array(items),
                    Some(Open::Object(members, _)) => Value::Object(members.into()),
                    None => unreachable!("an array or object was open"),
                };
            }
        }
    }
}

impl<'a> Reader<'a> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error { kind, offset: self.at }
    }

    /// Skips the whitespace of the grammar: space, tab, line feed and carriage return only.
    fn space(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.at) {
            self.at += 1;
        }
    }

    fn peek(&self) -> Result<u8, Error> {
        self.bytes.get(self.at).copied().ok_or_else(|| self.error(ErrorKind::UnexpectedEnd))
    }

    /// Whether the array or object just opened closes at once with `close`, which is consumed.
    fn close(&mut self, close: u8) -> bool {
        self.space();
        let closed = self.bytes.get(self.at) == Some(&close);
        self.at += usize::from(closed);
        closed
    }

    /// A member's name and the colon after it.
    fn name(&mut self) -> Result<Cow<'a, str>, Error> {
        if self.peek()? != b'"' {
            return Err(self.error(ErrorKind::Unexpected));
        }
        let name = self.string()?;
        self.space();
        if self.peek()? != b':' {
            return Err(self.error(ErrorKind::Unexpected));
        }
        self.at += 1;
        Ok(name)
    }

    fn string(&mut self) -> Result<Cow<'a, str>, Error> {
        let (s, end) =
            string::read(self.text, self.at + 1).map_err(|(kind, offset)| Error { kind, offset })?;
        self.at = end;
        Ok(s)
    }

    /// A string, a number or a literal.
    fn scalar(&mut self) -> Result<Value<'a>, Error> {
        let literal = |word: &[u8], value: Value<'a>, r: &mut Self| {
            if r.bytes[r.at..].starts_with(word) {
                r.at += word.len();
                Ok(value)
            } else {
                Err(r.error(ErrorKind::Unexpected))
            }
        };
        match self.peek()? {
            b'"' => Ok(Value::String(self.string()?)),
            b't' => literal(b"true", Value::Bool(true), self),
            b'f' => literal(b"false", Value::Bool(false), self),
            b'n' => literal(b"null", Value::Null, self),
            b'-' | b'0'..=b'9' => {
                let len = scan(&self.bytes[self.at..]).ok_or_else(|| self.error(ErrorKind::InvalidNumber))?;
                let number = Number::borrowed(&self.text[self.at..self.at + len]);
                self.at += len;
                Ok(Value::Number(number))
            }
            _ => Err(self.error(ErrorKind::Unexpected)),
        }
    }
}
