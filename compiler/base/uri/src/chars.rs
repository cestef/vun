//! The characters of each component (RFC 3986, section 2 and appendix A), as one table of classes
//! per byte, and the sets a component allows as written, built from them.

/// A class of characters, one bit of the table.
#[derive(Clone, Copy)]
struct Class(u8);

impl Class {
    const ALPHA: Self = Self(1);
    const DIGIT: Self = Self(1 << 1);
    /// `-`, `.`, `_`, `~`: with letters and digits, what is never escaped.
    const MARK: Self = Self(1 << 2);
    /// `!`, `$`, `&`, `'`, `(`, `)`, `*`, `+`, `,`, `;`, `=`.
    const SUB_DELIM: Self = Self(1 << 3);
    const COLON: Self = Self(1 << 4);
    const AT: Self = Self(1 << 5);
    const SLASH: Self = Self(1 << 6);
    const QUESTION: Self = Self(1 << 7);

    const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

const fn class_of(b: u8) -> Class {
    match b {
        b'a'..=b'z' | b'A'..=b'Z' => Class::ALPHA,
        b'0'..=b'9' => Class::DIGIT,
        b'-' | b'.' | b'_' | b'~' => Class::MARK,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'=' => Class::SUB_DELIM,
        b':' => Class::COLON,
        b'@' => Class::AT,
        b'/' => Class::SLASH,
        b'?' => Class::QUESTION,
        _ => Class(0),
    }
}

const TABLE: [u8; 256] = {
    let mut table = [0; 256];
    let mut b: u8 = 0;
    loop {
        table[b as usize] = class_of(b).0;
        if b == u8::MAX {
            break table;
        }
        b += 1;
    }
};

const UNRESERVED: Class = Class::ALPHA.with(Class::DIGIT).with(Class::MARK);
const PCHAR: Class = UNRESERVED.with(Class::SUB_DELIM).with(Class::COLON).with(Class::AT);

/// What a component, or a part of one, allows unescaped; every set but `Scheme` also allows
/// percent escapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Set {
    Scheme,
    UserInfo,
    /// A registered name: a host that is not an IP address.
    Host,
    Path,
    /// One segment of a path, whose `/` is escaped.
    Segment,
    Query,
    Fragment,
    /// A name or value of a query's pairs, whose `&`, `=`, `+` and `#` are escaped.
    Pair,
    /// What is never escaped anywhere.
    Unreserved,
}

impl Set {
    const fn classes(self) -> Class {
        match self {
            Self::Scheme => Class::ALPHA.with(Class::DIGIT),
            Self::UserInfo => UNRESERVED.with(Class::SUB_DELIM).with(Class::COLON),
            Self::Host => UNRESERVED.with(Class::SUB_DELIM),
            Self::Path => PCHAR.with(Class::SLASH),
            Self::Segment => PCHAR,
            Self::Query | Self::Fragment | Self::Pair => PCHAR.with(Class::SLASH).with(Class::QUESTION),
            Self::Unreserved => UNRESERVED,
        }
    }

    /// Whether `b` may stand unescaped in this set.
    #[must_use]
    pub const fn allows(self, b: u8) -> bool {
        match (self, b) {
            // A scheme's marks are `+`, `-` and `.`; a pair's delimiters are escaped.
            (Self::Scheme, b'+' | b'-' | b'.') => true,
            (Self::Scheme, _) => TABLE[b as usize] & Self::Scheme.classes().0 != 0,
            (Self::Pair, b'&' | b'=' | b'+') => false,
            _ => TABLE[b as usize] & self.classes().0 != 0,
        }
    }

    /// Whether this set allows percent escapes.
    #[must_use]
    pub const fn escapes(self) -> bool {
        !matches!(self, Self::Scheme)
    }
}
