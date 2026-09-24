//! Package versions: `major.minor.patch`, compared numerically.

use std::fmt;

use overt_diag::{DiagCx, Msg};
use overt_syntax::literal::{Decl, Literal};

use super::record::Record;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Separates the parts of a version.
const SEPARATOR: char = '.';

impl Version {
    /// Parses `major.minor.patch`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split(SEPARATOR).map(|p| {
            let canonical = p == "0" || !p.starts_with('0');
            p.parse().ok().filter(|_| canonical && p.bytes().all(|b| b.is_ascii_digit()))
        });
        let version = Self { major: parts.next()??, minor: parts.next()??, patch: parts.next()?? };
        parts.next().is_none().then_some(version)
    }

    pub(crate) fn read(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        match &decl.value {
            Literal::String(text) => Self::parse(text),
            _ => None,
        }
        .or_else(|| decl.invalid(Msg::new().text("a version ").code("\"major.minor.patch\""), cx))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{SEPARATOR}{}{SEPARATOR}{}", self.major, self.minor, self.patch)
    }
}
