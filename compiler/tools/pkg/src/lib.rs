//! Fetching package archives (spec 12, section 3). A mirror URL names a scheme and an archive
//! format, both looked up in tables, so that another scheme or format is one entry.

mod contents;
mod format;
mod mirror;
mod tar;
mod unpack;
mod zip;
mod zstd;

use std::error::Error;
use std::{fmt, io};

pub use contents::{Contents, Packed};
pub use format::{FORMATS, Format};
pub use mirror::{Locator, Mirror, Scheme, Schemes};
pub use tar::Tar;
pub use zip::Zip;
pub use zstd::Zstd;

/// Why a mirror could not provide an archive; the underlying error is its [`Error::source`].
#[derive(Debug)]
pub enum Failure {
    /// No scheme installed reads the URL; these are the ones that are.
    Scheme(Vec<&'static str>),
    /// No format in [`FORMATS`] matches the end of the URL.
    Format,
    Download(Box<dyn Error + Send + Sync>),
    Unpack(io::Error),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheme(schemes) => write!(f, "no supported scheme; supported: {}", schemes.join(", ")),
            Self::Format => {
                let suffixes: Vec<_> = FORMATS.iter().flat_map(|f| f.suffixes).copied().collect();
                write!(f, "no known archive format; supported: {}", suffixes.join(", "))
            }
            Self::Download(_) => f.write_str("cannot download the archive"),
            Self::Unpack(_) => f.write_str("cannot unpack the archive"),
        }
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheme(_) | Self::Format => None,
            Self::Download(e) => Some(e.as_ref()),
            Self::Unpack(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests;
