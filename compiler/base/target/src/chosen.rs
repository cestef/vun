//! A set of what a build is configured with, each once, in order.

use vun_codec::{Decode, Encode};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Chosen<T>(Vec<T>);

impl<T> Default for Chosen<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Copy + Ord> Chosen<T> {
    #[must_use]
    pub fn of(kinds: impl IntoIterator<Item = T>) -> Self {
        let mut kinds: Vec<T> = kinds.into_iter().collect();
        kinds.sort_unstable();
        kinds.dedup();
        Self(kinds)
    }

    #[must_use]
    pub fn contains(&self, kind: T) -> bool {
        self.0.contains(&kind)
    }

    pub fn iter(&self) -> impl Iterator<Item = T> {
        self.0.iter().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
