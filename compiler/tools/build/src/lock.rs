//! The lockfile, `overt.lock` (spec 12, section 4): every version the graph requires, with the
//! hash it was verified against, one independent line per entry, sorted, so that merges never
//! conflict. The selected version of an identity is the largest recorded.

use std::fmt;

use overt_hash::Hash;

use crate::manifest::{Identity, Version};

/// The lockfile's name, next to the manifest.
pub const FILE: &str = "overt.lock";

/// A lockfile: the required packages, sorted by name, version and identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lock {
    entries: Vec<Locked>,
}

/// One required package.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Locked {
    /// The name the first dependent to require it binds it to.
    pub name: String,
    pub version: Version,
    /// `None` for a foreign package, which is identified by its hash alone.
    pub identity: Option<Identity>,
    pub hash: Hash,
    /// The dependent that requires it, when that is not the project.
    pub via: Option<String>,
}

/// Why a lockfile does not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// The first line is not the header of a known version.
    Header,
    /// A line, by its number from 1, is not an entry.
    Line(usize),
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header => write!(f, "the first line is not `{HEADER}`"),
            Self::Line(n) => write!(f, "line {n} is not an entry"),
        }
    }
}

impl std::error::Error for Malformed {}

const HEADER: &str = "# overt.lock 1 (written by `overt pkg`; do not edit)";
/// Starts an entry line.
const DEP: &str = "dep";
/// Stands for the identity of a foreign package.
const FOREIGN: &str = "foreign";
/// Introduces the dependent that requires an entry.
const VIA: &str = "via";

impl Lock {
    /// The lock of `entries`, in their canonical order.
    #[must_use]
    pub fn new(mut entries: Vec<Locked>) -> Self {
        entries.sort();
        entries.dedup();
        Self { entries }
    }

    #[must_use]
    pub fn entries(&self) -> &[Locked] {
        &self.entries
    }

    /// The entry selected for `identity`: the largest version required.
    #[must_use]
    pub fn selected(&self, identity: Identity) -> Option<&Locked> {
        self.entries.iter().filter(|e| e.identity == Some(identity)).max_by_key(|e| e.version)
    }

    /// Reads the text form.
    ///
    /// # Errors
    /// Returns the first line that does not read.
    pub fn parse(text: &str) -> Result<Self, Malformed> {
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return Err(Malformed::Header);
        }
        let entries = lines
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(i, line)| Locked::parse(line).ok_or(Malformed::Line(i + 2)))
            .collect::<Result<_, _>>()?;
        Ok(Self::new(entries))
    }
}

impl Locked {
    fn parse(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        (words.next()? == DEP).then_some(())?;
        let name = words.next()?.to_owned();
        let version = Version::parse(words.next()?)?;
        let identity = match words.next()? {
            FOREIGN => None,
            text => Some(Identity::parse(text)?),
        };
        let hash = Hash::parse(words.next()?)?;
        let via = match words.next() {
            Some(VIA) => Some(words.next()?.to_owned()),
            Some(_) => return None,
            None => None,
        };
        words.next().is_none().then_some(Self { name, version, identity, hash, via })
    }
}

impl fmt::Display for Lock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{HEADER}")?;
        self.entries.iter().try_for_each(|e| writeln!(f, "{e}"))
    }
}

impl fmt::Display for Locked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{DEP} {} {} ", self.name, self.version)?;
        match self.identity {
            Some(identity) => write!(f, "{identity}")?,
            None => f.write_str(FOREIGN)?,
        }
        write!(f, " {}", self.hash)?;
        match &self.via {
            Some(via) => write!(f, " {VIA} {via}"),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests;
