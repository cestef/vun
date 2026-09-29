//! The trees `tree-sitter parse` prints, read back: each named node with its field, where it
//! starts and ends, and its named children; `ERROR` and `MISSING` nodes as the parser names them.

use std::fmt;

/// Where in a file: a line and a byte column, from zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub row: u32,
    pub column: u32,
}

/// A named node of a printed tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// Its kind; `ERROR` for what the parser skipped.
    pub kind: String,
    /// Whether the parser supposed it, absent from the source: `(MISSING kind ...)`.
    pub missing: bool,
    /// The field of its parent it stands in.
    pub field: Option<String>,
    pub start: Point,
    pub end: Point,
    pub children: Vec<Node>,
}

/// The trees of a printed parse, one per file, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    pub roots: Vec<Node>,
}

/// Why printed text is not a tree: what was expected, and the byte where it was not found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed {
    pub expected: &'static str,
    pub at: usize,
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected {} at byte {}", self.expected, self.at)
    }
}

impl std::error::Error for Malformed {}

/// Marks a node the parser supposed.
const MISSING: &str = "MISSING";

impl Node {
    /// Whether this node or one below it is an error or missing.
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.missing || self.kind == "ERROR" || self.children.iter().any(Self::has_error)
    }
}

impl Tree {
    /// The trees in `text`, as `tree-sitter parse` prints them; lines outside a tree, as the
    /// statistics of a file, are skipped.
    ///
    /// # Errors
    /// Returns where a tree is not well formed.
    pub fn read(text: &str) -> Result<Self, Malformed> {
        let mut reader = Reader { text: text.as_bytes(), at: 0 };
        let mut roots = Vec::new();
        loop {
            reader.skip_to_tree();
            if reader.at >= reader.text.len() {
                return Ok(Self { roots });
            }
            roots.push(reader.node(None)?);
        }
    }
}

struct Reader<'t> {
    text: &'t [u8],
    at: usize,
}

impl Reader<'_> {
    /// Past lines not starting a tree.
    fn skip_to_tree(&mut self) {
        while self.at < self.text.len() && self.text[self.at] != b'(' {
            while self.at < self.text.len() && self.text[self.at] != b'\n' {
                self.at += 1;
            }
            self.at += 1;
        }
    }

    fn space(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn expect(&mut self, byte: u8, expected: &'static str) -> Result<(), Malformed> {
        self.space();
        if self.text.get(self.at) == Some(&byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(Malformed { expected, at: self.at })
        }
    }

    /// A word: a kind, a field name or a number.
    fn word(&mut self) -> &str {
        self.space();
        let start = self.at;
        while self.text.get(self.at).is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_') {
            self.at += 1;
        }
        std::str::from_utf8(&self.text[start..self.at]).unwrap_or_default()
    }

    /// A quoted token, as `MISSING` names an anonymous one.
    fn quoted(&mut self) -> Result<String, Malformed> {
        self.expect(b'"', "a quoted token")?;
        let start = self.at;
        while self.text.get(self.at).is_some_and(|&b| b != b'"') {
            self.at += usize::from(self.text[self.at] == b'\\') + 1;
        }
        let token = String::from_utf8_lossy(&self.text[start..self.at.min(self.text.len())]).into_owned();
        self.expect(b'"', "the end of a quoted token")?;
        Ok(token)
    }

    fn number(&mut self) -> Result<u32, Malformed> {
        let at = self.at;
        self.word().parse().map_err(|_| Malformed { expected: "a number", at })
    }

    /// `[row, column]`.
    fn point(&mut self) -> Result<Point, Malformed> {
        self.expect(b'[', "a point")?;
        let row = self.number()?;
        self.expect(b',', "a comma in a point")?;
        let column = self.number()?;
        self.expect(b']', "the end of a point")?;
        Ok(Point { row, column })
    }

    /// `(kind [start] - [end] children...)`, the opening parenthesis next.
    fn node(&mut self, field: Option<String>) -> Result<Node, Malformed> {
        self.expect(b'(', "a node")?;
        let mut kind = self.word().to_owned();
        let missing = kind == MISSING;
        if missing {
            self.space();
            kind =
                if self.text.get(self.at) == Some(&b'"') { self.quoted()? } else { self.word().to_owned() };
        }
        let start = self.point()?;
        self.expect(b'-', "a range")?;
        let end = self.point()?;
        let mut children = Vec::new();
        loop {
            self.space();
            match self.text.get(self.at) {
                Some(b')') => {
                    self.at += 1;
                    return Ok(Node { kind, missing, field, start, end, children });
                }
                Some(b'(') => children.push(self.node(None)?),
                Some(_) => {
                    let name = self.word().to_owned();
                    self.expect(b':', "a field")?;
                    children.push(self.node(Some(name))?);
                }
                None => return Err(Malformed { expected: "the end of a node", at: self.at }),
            }
        }
    }
}
