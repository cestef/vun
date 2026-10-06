//! Formatters as components (decision 498): a [`Formatter`] lays source text out (spec 13,
//! section 5), as `vun fmt` and the language server's formatting ask. [`Formatters`] is the
//! interface a formatter library is served through, [`Remote`] the `Formatter` the host calls it
//! by. The formatter is the same crate linked in or loaded, and asks nothing of the host.

mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt;

use vun_codec::{Decode, Encode};
use vun_component::Cause;

pub use protocol::{Call, Reply};
pub use remote::Remote;
pub use serve::Formatters;

/// The columns a line has before the groups on it are broken, where nothing says otherwise.
pub const WIDTH: usize = 110;

pub trait Formatter: Send + Sync {
    /// What `vun` calls it: `vun`.
    fn name(&self) -> &'static str;

    /// `text` laid out, with lines of at most `width` columns where they can be broken.
    ///
    /// # Errors
    /// Returns why it is not laid out.
    fn layout(&self, text: &str, width: usize) -> Result<String, Unlaid>;
}

/// Why a text is not laid out.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Unlaid {
    /// It does not parse: what is wrong with it is for checking to report.
    Unparsed,
    /// The component serving the formatter failed.
    Faulted(Cause),
}

impl fmt::Display for Unlaid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unparsed => f.write_str("the text does not parse"),
            Self::Faulted(_) => f.write_str("the formatter failed"),
        }
    }
}

impl Error for Unlaid {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unparsed => None,
            Self::Faulted(cause) => Some(cause),
        }
    }
}
