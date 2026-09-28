//! The URLs a download requests: parsed, over `https`, and redirects read against them.

use std::fmt;

use ureq::http::uri::{Authority, Parts, PathAndQuery, Scheme, Uri};

use crate::Unfetched;

/// A URL over `https`, the only scheme requested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Secure(Uri);

/// What starts a reference naming another host, and ends a fragment, never sent (RFC 3986,
/// sections 4.2 and 3.5).
const NETWORK: &str = "//";
const FRAGMENT: char = '#';
/// What starts a query, and separates the segments of a path.
const QUERY: char = '?';
const SEGMENT: char = '/';

impl Secure {
    /// What starts the URLs of the scheme, as sources write them.
    pub const PREFIX: &str = "https://";

    /// `text`, a URL a source names.
    ///
    /// # Errors
    /// Returns that it does not parse, or is not over `https`.
    pub fn parse(text: &str) -> Result<Self, Unfetched> {
        Self::checked(Uri::try_from(text)?)
    }

    #[must_use]
    pub fn uri(&self) -> &Uri {
        &self.0
    }

    /// The target of a redirect from this URL, `location` read against it (RFC 3986,
    /// section 5.2), without removing dot segments, which the server resolves.
    ///
    /// # Errors
    /// Returns that the target does not parse, or is not over `https`.
    pub fn follow(&self, location: &[u8]) -> Result<Self, Unfetched> {
        let text = str::from_utf8(location).map_err(|e| Unfetched::Unparsed(Box::new(e)))?;
        let text = text.split(FRAGMENT).next().unwrap_or_default();
        if Self::absolute(text) {
            return Self::checked(Uri::try_from(text)?);
        }
        let (authority, target) = match text.strip_prefix(NETWORK) {
            Some(rest) => {
                let (authority, path) = rest.split_at(rest.find([SEGMENT, QUERY]).unwrap_or(rest.len()));
                (Authority::try_from(authority)?, path.to_owned())
            }
            None => {
                (self.0.authority().cloned().ok_or(Unfetched::Insecure(self.0.clone()))?, self.path(text))
            }
        };
        let mut parts = Parts::default();
        parts.scheme = Some(Scheme::HTTPS);
        parts.authority = Some(authority);
        parts.path_and_query = Some(PathAndQuery::try_from(target)?);
        Self::checked(Uri::from_parts(parts)?)
    }

    /// Whether `reference` starts with a scheme (RFC 3986, section 3.1).
    fn absolute(reference: &str) -> bool {
        reference.split_once(':').is_some_and(|(scheme, _)| {
            let mut chars = scheme.chars();
            chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        })
    }

    /// The path and query a reference on this URL's host names: from the root, this path with
    /// another query, or relative to this path's last segment.
    fn path(&self, reference: &str) -> String {
        let here = self.0.path();
        if reference.starts_with(SEGMENT) {
            reference.to_owned()
        } else if reference.is_empty() || reference.starts_with(QUERY) {
            format!("{here}{reference}")
        } else {
            let dir = here.rfind(SEGMENT).map_or("", |end| &here[..=end]);
            format!("{dir}{reference}")
        }
    }

    /// `uri` if it is over `https` and names a host.
    fn checked(uri: Uri) -> Result<Self, Unfetched> {
        if uri.scheme() == Some(&Scheme::HTTPS) && uri.authority().is_some() {
            Ok(Self(uri))
        } else {
            Err(Unfetched::Insecure(uri))
        }
    }
}

impl fmt::Display for Secure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
