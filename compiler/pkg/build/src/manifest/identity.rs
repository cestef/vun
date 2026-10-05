//! Package identity (spec 12, section 3): a package is its `id` and major version; different
//! majors are different packages and coexist.

use std::fmt;

use vun_span::Span;

use super::{Manifest, Source};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identity {
    pub id: u64,
    pub major: u32,
}

/// Separates the id from the major version in the text form: `6a8091f57c7f07ff/1`.
const SEPARATOR: char = '/';
/// Hex digits of an id in the text form.
const ID_DIGITS: usize = 16;

impl Identity {
    /// Reads `<id in hex>/<major>`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (id, major) = text.split_once(SEPARATOR)?;
        let hex =
            id.len() == ID_DIGITS && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        let id = u64::from_str_radix(id, 16).ok().filter(|_| hex)?;
        let major = major.parse().ok().filter(|m: &u32| m.to_string() == major)?;
        Some(Self { id, major })
    }
}

impl Identity {
    /// The parts of its text form: the id in hexadecimal, then the major version.
    #[must_use]
    pub fn parts(&self) -> [String; 2] {
        [format!("{:0width$x}", self.id, width = ID_DIGITS), self.major.to_string()]
    }
}

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [id, major] = self.parts();
        write!(f, "{id}{SEPARATOR}{major}")
    }
}

impl Manifest {
    #[must_use]
    pub fn identity(&self) -> Identity {
        Identity { id: self.id, major: self.version.major }
    }

    /// Where it declares a dependency or a replacement by path or from the library, which only a
    /// package on the user's disk may: a stored one names every package by hash.
    pub fn by_path(&self) -> impl Iterator<Item = Span> + '_ {
        let deps = self.deps.iter().map(|d| (&d.source, d.span));
        let replace = self.replace.iter().map(|r| (&r.source, r.span));
        deps.chain(replace)
            .filter(|(source, _)| matches!(source, Source::Path(_) | Source::Library(_)))
            .map(|(_, span)| span)
    }

    /// The name of its release, `hal-1.0.0`: the stem of its archives and the directory foreign
    /// formats hold its files in.
    #[must_use]
    pub fn release(&self) -> String {
        format!("{}-{}", self.name, self.version)
    }
}
