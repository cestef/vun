//! Messages framed as the protocol frames them: headers, each a line ending in CRLF, the
//! `Content-Length` one giving how many bytes of UTF-8 JSON follow the blank line after them.

use std::fmt;
use std::io::{self, BufRead, Write};

/// The header giving the length of a message's body.
const LENGTH: &str = "Content-Length";
/// What ends a header line.
const LINE_END: &str = "\r\n";

/// Why a message could not be read.
#[derive(Debug)]
pub enum Unframed {
    Io(io::Error),
    /// The headers give no length, or not a number.
    Length,
    /// The body is not UTF-8.
    Utf8,
}

impl fmt::Display for Unframed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => f.write_str("the message could not be read"),
            Self::Length => write!(f, "the message has no `{LENGTH}` header"),
            Self::Utf8 => f.write_str("the message is not UTF-8"),
        }
    }
}

impl std::error::Error for Unframed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Length | Self::Utf8 => None,
        }
    }
}

/// The messages a client sends.
pub(crate) struct Incoming<R>(pub(crate) R);

impl<R: BufRead> Incoming<R> {
    /// The next message's body; `None` once the client closed the stream.
    pub(crate) fn next(&mut self) -> Result<Option<String>, Unframed> {
        let mut length = None;
        let mut line = String::new();
        loop {
            line.clear();
            if self.0.read_line(&mut line).map_err(Unframed::Io)? == 0 {
                return Ok(None);
            }
            let header = line.trim_end_matches(LINE_END);
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case(LENGTH)
            {
                length = Some(value.trim().parse::<usize>().map_err(|_| Unframed::Length)?);
            }
        }
        let mut body = vec![0; length.ok_or(Unframed::Length)?];
        self.0.read_exact(&mut body).map_err(Unframed::Io)?;
        String::from_utf8(body).map(Some).map_err(|_| Unframed::Utf8)
    }
}

/// The messages the server sends.
pub(crate) struct Outgoing<W>(pub(crate) W);

impl<W: Write> Outgoing<W> {
    pub(crate) fn send(&mut self, body: &str) -> io::Result<()> {
        write!(self.0, "{LENGTH}: {}{LINE_END}{LINE_END}{body}", body.len())?;
        self.0.flush()
    }
}
