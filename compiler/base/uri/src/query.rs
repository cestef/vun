//! The query of a URI (RFC 3986, section 3.4), and the `name=value` pairs it most often holds,
//! separated by `&`.

use crate::percent::Encoded;

/// A URI's query, as written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Query<'a>(pub(crate) Encoded<'a>);

/// What separates pairs, and a pair's name from its value.
const PAIRS: char = '&';
const VALUE: char = '=';

impl<'a> Query<'a> {
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0.as_str()
    }

    #[must_use]
    pub const fn encoded(self) -> Encoded<'a> {
        self.0
    }

    /// Its pairs in order, each a name and, after `=`, a value; empty pieces are skipped.
    pub fn pairs(self) -> impl Iterator<Item = (Encoded<'a>, Option<Encoded<'a>>)> {
        self.as_str().split(PAIRS).filter(|p| !p.is_empty()).map(|pair| match pair.split_once(VALUE) {
            Some((name, value)) => (Encoded(name), Some(Encoded(value))),
            None => (Encoded(pair), None),
        })
    }

    /// The value of the first pair whose decoded name is `name`.
    #[must_use]
    pub fn get(self, name: &str) -> Option<Encoded<'a>> {
        self.pairs().find(|(n, _)| *n.decode() == *name.as_bytes()).and_then(|(_, v)| v)
    }
}
