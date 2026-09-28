//! Content hashes as output shows them.

use vun_hash::Hash;
use vun_term::{Style, Text};

/// How much of a content hash a line shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::commands) enum Shown {
    /// Enough to tell stored packages apart, as jj shows change ids.
    Short,
    Full,
}

/// The digest characters of a short hash.
const SHORT: usize = 12;

pub(in crate::commands) trait HashText {
    /// Its algorithm dimmed, its digest as an id.
    fn text(&self, shown: Shown) -> Text;
}

impl HashText for Hash {
    fn text(&self, shown: Shown) -> Text {
        let digest = self.digest();
        let digest = if shown == Shown::Short { digest.chars().take(SHORT).collect() } else { digest };
        let algorithm = format!("{}{}", self.algorithm().name(), Self::SEPARATOR);
        Text::of(Style::Dim, algorithm).styled(Style::Id, digest)
    }
}
