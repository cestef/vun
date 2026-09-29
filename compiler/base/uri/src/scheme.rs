//! The scheme of a URI (RFC 3986, section 3.1), compared without case, and what the schemes this
//! crate knows imply for normalization (section 6.2.3).

use std::fmt;

/// A URI's scheme, as written.
#[derive(Clone, Copy, Debug)]
pub struct Scheme<'a>(pub(crate) &'a str);

/// What a scheme implies: its port when none is written, and whether its paths are hierarchical,
/// an empty one meaning `/` after an authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    pub name: &'static str,
    pub port: Option<u16>,
    pub rooted: bool,
}

impl Known {
    pub const FILE: Self = Self { name: "file", port: None, rooted: true };
    pub const HTTP: Self = Self { name: "http", port: Some(80), rooted: true };
    pub const HTTPS: Self = Self { name: "https", port: Some(443), rooted: true };

    /// The schemes known, one row each.
    pub const ALL: &[Self] = &[
        Self::FILE,
        Self::HTTP,
        Self::HTTPS,
        Self { name: "ws", port: Some(80), rooted: true },
        Self { name: "wss", port: Some(443), rooted: true },
        Self { name: "ftp", port: Some(21), rooted: true },
        Self { name: "ssh", port: Some(22), rooted: true },
        Self { name: "git", port: Some(9418), rooted: true },
    ];
}

impl<'a> Scheme<'a> {
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }

    /// Whether it is `name`, compared without case.
    #[must_use]
    pub fn is(self, name: &str) -> bool {
        self.0.eq_ignore_ascii_case(name)
    }

    /// What it implies, when it is a scheme this crate knows.
    #[must_use]
    pub fn known(self) -> Option<&'static Known> {
        Known::ALL.iter().find(|k| self.is(k.name))
    }
}

impl PartialEq for Scheme<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.is(other.0)
    }
}

impl Eq for Scheme<'_> {}

impl fmt::Display for Scheme<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
