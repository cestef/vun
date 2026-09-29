//! Why a download stopped before its body.

use std::error::Error;
use std::fmt;

use ureq::http::StatusCode;
use vun_fetch::Failure;
use vun_uri::{Invalid, Known, Uri};

#[derive(Debug)]
pub enum Unfetched {
    /// The server answered with a status that is not a success.
    Refused(StatusCode),
    /// A redirect named no target.
    Unlocated,
    /// A URL, requested or redirected to, that does not parse.
    Unparsed(Invalid),
    /// A redirect whose location is not UTF-8.
    Garbled,
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
            Self::Garbled => f.write_str("redirected to a location that is not text"),
            Self::Insecure(uri) => write!(f, "`{uri}` is not an `{}` URL naming a host", Known::HTTPS.name),
            Self::Endless => f.write_str("redirected too many times"),
        }
    }
}

impl Error for Unfetched {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unparsed(e) => Some(e),
            Self::Refused(_) | Self::Unlocated | Self::Garbled | Self::Insecure(_) | Self::Endless => None,
        }
    }
}

impl From<Unfetched> for Failure {
    fn from(e: Unfetched) -> Self {
        Self::Download(Box::new(e))
    }
}
