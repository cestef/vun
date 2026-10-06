//! An error that crossed the boundary: what a component's failure said, each cause below it.

use std::error::Error;
use std::fmt;

use vun_codec::{Decode, Encode};

/// A failure as its messages, outermost first. A component's own error types do not cross, so
/// what the host holds is the chain they said, reached through [`Error::source`] as theirs is.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct Cause {
    message: String,
    source: Option<Box<Cause>>,
}

impl Cause {
    /// What `error` and each of its sources say.
    #[must_use]
    pub fn of(error: &(dyn Error + 'static)) -> Self {
        Self { message: error.to_string(), source: error.source().map(|s| Box::new(Self::of(s))) }
    }

    /// The messages, outermost first.
    pub fn messages(&self) -> impl Iterator<Item = &str> {
        std::iter::successors(Some(self), |c| c.source.as_deref()).map(|c| c.message.as_str())
    }
}

impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for Cause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|s| s as _)
    }
}
