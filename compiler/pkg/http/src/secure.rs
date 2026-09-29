//! The URLs a download requests: parsed, over `https`, and redirects read against them.

use std::fmt;

use vun_uri::{Known, Reference, Uri};

use crate::Unfetched;

/// A URL over `https` naming a host, the only kind requested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Secure(Uri);

impl Secure {
    /// `text`, a URL a source names.
    ///
    /// # Errors
    /// Returns that it does not parse, or is not over `https`.
    pub fn parse(text: &str) -> Result<Self, Unfetched> {
        let reference = Reference::parse(text).map_err(Unfetched::Unparsed)?;
        Self::checked(reference.try_into().map_err(Unfetched::Unparsed)?)
    }

    /// A URL already read.
    ///
    /// # Errors
    /// Returns that it is not over `https`.
    pub fn of(uri: &Uri) -> Result<Self, Unfetched> {
        Self::checked(uri.clone())
    }

    #[must_use]
    pub fn uri(&self) -> &Uri {
        &self.0
    }

    /// The target of a redirect from this URL, `location` read against it (RFC 3986, section
    /// 5.2), without its fragment, which is never sent.
    ///
    /// # Errors
    /// Returns that the target does not parse, or is not over `https`.
    pub fn follow(&self, location: &[u8]) -> Result<Self, Unfetched> {
        let text = str::from_utf8(location).map_err(|_| Unfetched::Garbled)?;
        let reference = Reference::parse(text).map_err(Unfetched::Unparsed)?;
        Self::checked(self.0.resolve(&reference).without_fragment())
    }

    /// `uri` if it is over `https` and names a host.
    fn checked(uri: Uri) -> Result<Self, Unfetched> {
        let named = uri.host().is_some_and(|h| !h.as_str().is_empty());
        if uri.scheme().is(Known::HTTPS.name) && named {
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
