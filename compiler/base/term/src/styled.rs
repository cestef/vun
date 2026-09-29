//! Lines for [`Format::Styled`](crate::Format::Styled): what people read, each line a JSON
//! object of its pieces and their styles, for tools that draw vun's output themselves.

use std::io::{self, Write};

use vun_data::{Object, ToData};

use crate::{Piece, Record, Stream, Style, Text};

/// A line as people read it, and the stream it went to.
struct Line<'p> {
    stream: Stream,
    pieces: &'p [Piece],
}

impl Record for Line<'_> {
    fn kind(&self) -> &'static str {
        "line"
    }

    fn text(&self) -> Text {
        self.pieces.iter().fold(Text::new(), |text, p| text.maybe(p.style, p.text.clone()))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("stream", self.stream.to_data()).with("pieces", self.pieces.to_data())
    }
}

/// The line being written on a stream: its pieces so far, and bytes written plain that do not
/// end a line yet.
pub(crate) struct Lines {
    stream: Stream,
    pieces: Vec<Piece>,
    bytes: Vec<u8>,
}

impl Lines {
    pub(crate) const fn new(stream: Stream) -> Self {
        Self { stream, pieces: Vec::new(), bytes: Vec::new() }
    }

    /// Adds `text`, writing each line it ends to `w`.
    pub(crate) fn print(&mut self, w: &mut dyn Write, text: &Text) -> io::Result<()> {
        self.settle();
        for piece in text.pieces() {
            let mut parts = piece.text.split('\n');
            if let Some(first) = parts.next() {
                self.push(piece.style, first);
            }
            for part in parts {
                self.end(w)?;
                self.push(piece.style, part);
            }
        }
        Ok(())
    }

    /// Adds bytes written plain, as a renderer styling its own output writes them.
    pub(crate) fn write(&mut self, w: &mut dyn Write, buf: &[u8]) -> io::Result<()> {
        let mut rest = buf;
        while let Some(at) = rest.iter().position(|&b| b == b'\n') {
            self.bytes.extend_from_slice(&rest[..at]);
            self.settle();
            self.end(w)?;
            rest = &rest[at + 1..];
        }
        self.bytes.extend_from_slice(rest);
        Ok(())
    }

    /// Writes the line begun, if any, as a stream ends without a newline.
    pub(crate) fn finish(&mut self, w: &mut dyn Write) -> io::Result<()> {
        self.settle();
        if self.pieces.is_empty() { Ok(()) } else { self.end(w) }
    }

    fn push(&mut self, style: Option<Style>, text: &str) {
        if !text.is_empty() {
            self.pieces.push(Piece { style, text: text.to_owned().into() });
        }
    }

    // Plain bytes become a piece of their own.
    fn settle(&mut self) {
        if !self.bytes.is_empty() {
            let bytes = std::mem::take(&mut self.bytes);
            self.push(None, &String::from_utf8_lossy(&bytes));
        }
    }

    fn end(&mut self, w: &mut dyn Write) -> io::Result<()> {
        let line = Line { stream: self.stream, pieces: &self.pieces };
        writeln!(w, "{}", vun_data::json::Json(&line.json()))?;
        self.pieces.clear();
        Ok(())
    }
}
