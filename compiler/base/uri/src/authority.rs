//! The authority of a URI (RFC 3986, section 3.2): user information, a host and a port.

use std::net::IpAddr;

use crate::parts::{AuthorityParts, HostKind};
use crate::percent::Encoded;

/// A URI's authority, `[userinfo@]host[:port]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Authority<'a> {
    text: &'a str,
    parts: AuthorityParts,
}

/// A host: its text as written and what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Host<'a> {
    text: &'a str,
    kind: HostKind,
}

/// The name a host takes for this machine (RFC 8089, section 2).
const LOCALHOST: &str = "localhost";

impl<'a> Authority<'a> {
    pub(crate) const fn new(text: &'a str, parts: AuthorityParts) -> Self {
        Self { text, parts }
    }

    /// The authority as written, without its `//`.
    #[must_use]
    pub fn as_str(&self) -> &'a str {
        self.parts.whole.of(self.text)
    }

    #[must_use]
    pub fn userinfo(&self) -> Option<Encoded<'a>> {
        self.parts.userinfo.map(|p| Encoded(p.of(self.text)))
    }

    #[must_use]
    pub fn host(&self) -> Host<'a> {
        Host { text: self.parts.host.of(self.text), kind: self.parts.kind }
    }

    /// The port, when one is written.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.port_str()?.parse().ok()
    }

    /// The port's digits as written, possibly none after its `:`.
    #[must_use]
    pub fn port_str(&self) -> Option<&'a str> {
        self.parts.port.map(|digits| digits.of(self.text))
    }
}

impl<'a> Host<'a> {
    /// The host as written, an IP literal with its brackets.
    #[must_use]
    pub const fn as_str(&self) -> &'a str {
        self.text
    }

    #[must_use]
    pub const fn kind(&self) -> HostKind {
        self.kind
    }

    /// The registered name, when the host is one.
    #[must_use]
    pub fn name(&self) -> Option<Encoded<'a>> {
        (self.kind == HostKind::Name).then_some(Encoded(self.text))
    }

    /// The IP address, when the host is one.
    #[must_use]
    pub fn ip(&self) -> Option<IpAddr> {
        match self.kind {
            HostKind::Ipv4 => self.text.parse().ok().map(IpAddr::V4),
            HostKind::Ipv6 => self.text.get(1..self.text.len() - 1)?.parse().ok().map(IpAddr::V6),
            HostKind::Name | HostKind::Future => None,
        }
    }

    /// Whether the host names this machine as a file URI does: empty, or `localhost`.
    #[must_use]
    pub fn is_local(&self) -> bool {
        self.text.is_empty() || self.text.eq_ignore_ascii_case(LOCALHOST)
    }
}
