//! Why text is not a URI, or a URI not a local file: typed, with where it went wrong.

use std::fmt;

use crate::chars::Set;

/// Text that is not a URI or reference: why, at which byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invalid {
    pub why: Why,
    pub at: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// A byte the component does not allow unescaped.
    Char(Set),
    /// A `%` not followed by two hexadecimal digits.
    Escape,
    /// No scheme, where a URI needs one: a relative reference.
    Relative,
    /// A scheme that does not start with a letter.
    Scheme,
    /// A port that is not a number up to 65535.
    Port,
    /// An IP literal holding neither an IPv6 address nor an `IPvFuture`.
    IpLiteral,
    /// An IP literal whose `]` is missing, or followed by something other than a port.
    Bracket,
    /// A path after an authority that does not start with `/`.
    AuthorityPath,
    /// A path starting with `//` without an authority, which would read as one.
    PathStart,
    /// Text longer than a reference may be.
    Length,
    /// The parts recorded of a URI are not those its text parses into.
    Parts,
}

impl Invalid {
    #[must_use]
    pub const fn new(why: Why, at: usize) -> Self {
        Self { why, at }
    }

    /// This error, found in a part of the text that starts at byte `start`.
    #[must_use]
    pub const fn shifted(self, start: usize) -> Self {
        Self { at: self.at + start, ..self }
    }
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(set) => write!(f, "a character {set} does not allow unescaped"),
            Self::Escape => f.write_str("a `%` not followed by two hexadecimal digits"),
            Self::Relative => f.write_str("no scheme, as in a relative reference"),
            Self::Scheme => f.write_str("a scheme that does not start with a letter"),
            Self::Port => f.write_str("a port that is not a number up to 65535"),
            Self::IpLiteral => f.write_str("an IP literal that is neither an IPv6 address nor an IPvFuture"),
            Self::Bracket => f.write_str("an IP literal not closed by `]` at the end of its host"),
            Self::AuthorityPath => f.write_str("a path after an authority that does not start with `/`"),
            Self::PathStart => f.write_str("a path starting with `//` without an authority"),
            Self::Length => f.write_str("text longer than 4 GiB"),
            Self::Parts => f.write_str("parts recorded apart from those the text parses into"),
        }
    }
}

impl fmt::Display for Set {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Scheme => "a scheme",
            Self::UserInfo => "user information",
            Self::Host => "a host name",
            Self::Path => "a path",
            Self::Segment => "a path segment",
            Self::Query => "a query",
            Self::Fragment => "a fragment",
            Self::Pair => "a query's name or value",
            Self::Unreserved => "the unreserved characters",
        })
    }
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.why, self.at)
    }
}

impl std::error::Error for Invalid {}

/// A URI that names no local file, or a path that no file URI names (RFC 8089).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotFile {
    /// The scheme is not `file`.
    Scheme,
    /// The host is another machine's, which this system does not name as a path.
    Host,
    /// The path is relative, where a file URI names an absolute one.
    Relative,
    /// The path's bytes are not a path of this system.
    Encoding,
}

impl fmt::Display for NotFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Scheme => "the URI's scheme is not `file`",
            Self::Host => "the URI names a file on another machine",
            Self::Relative => "the path is relative",
            Self::Encoding => "the path's bytes are not a path of this system",
        })
    }
}

impl std::error::Error for NotFile {}
