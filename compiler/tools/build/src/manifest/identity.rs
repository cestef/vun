//! Package identity (spec 12, section 3): a package is its `id` and major version; different
//! majors are different packages and coexist.

use std::fmt;

use super::Manifest;

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

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:0width$x}{SEPARATOR}{}", self.id, self.major, width = ID_DIGITS)
    }
}

impl Manifest {
    #[must_use]
    pub fn identity(&self) -> Identity {
        Identity { id: self.id, major: self.version.major }
    }

    /// The name of its release, `hal-1.0.0`: the stem of its archives and the directory foreign
    /// formats hold its files in.
    #[must_use]
    pub fn release(&self) -> String {
        format!("{}-{}", self.name, self.version)
    }
}
