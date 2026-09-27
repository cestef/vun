//! Why a download stopped before its body.

use std::error::Error;
use std::fmt;

use ureq::http::StatusCode;
use ureq::http::uri::{InvalidUri, InvalidUriParts, Uri};
use vun_pkg::Failure;

use crate::Secure;

#[derive(Debug)]
pub enum Unfetched {
    /// The server answered with a status that is not a success.
    Refused(StatusCode),
    /// A redirect named no target.
    Unlocated,
    /// A URL, requested or redirected to, that does not parse.
    Unparsed(Box<dyn Error + Send + Sync>),
    /// A URL whose scheme is not `https`, which is never requested.
    Insecure(Uri),
    /// More redirects than are followed.
    Endless,
}

impl fmt::Display for Unfetched {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(status) => write!(f, "the server answered {status}"),
            Self::Unlocated => f.write_str("redirected without a location"),
            Self::Unparsed(_) => f.write_str("not a URL"),
            Self::Insecure(uri) => write!(f, "`{uri}` is not an `{}` URL", Secure::PREFIX),
            Self::Endless => f.write_str("redirected too many times"),
        }
    }
}

impl Error for Unfetched {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unparsed(e) => Some(e.as_ref()),
            Self::Refused(_) | Self::Unlocated | Self::Insecure(_) | Self::Endless => None,
        }
    }
}

impl From<InvalidUri> for Unfetched {
    fn from(e: InvalidUri) -> Self {
        Self::Unparsed(Box::new(e))
    }
}

impl From<InvalidUriParts> for Unfetched {
    fn from(e: InvalidUriParts) -> Self {
        Self::Unparsed(Box::new(e))
    }
}

impl From<Unfetched> for Failure {
    fn from(e: Unfetched) -> Self {
        Self::Download(Box::new(e))
    }
}
