//! Sets of indices as bits: one bit per index, words grown as needed. A flow over a function's
//! locals or loans joins and copies these as words, where a tree set would allocate per element.

use std::fmt;
use std::marker::PhantomData;

use crate::Idx;

pub(crate) const WORD: usize = u64::BITS as usize;

/// A set of `I`, iterated in ascending order.
#[derive(Clone)]
pub struct BitSet<I: Idx> {
    words: Vec<u64>,
    _index: PhantomData<fn(&I)>,
}

impl<I: Idx> Default for BitSet<I> {
    fn default() -> Self {
        Self { words: Vec::new(), _index: PhantomData }
    }
}

impl<I: Idx> BitSet<I> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `i`, returning whether it was absent.
    pub fn insert(&mut self, i: I) -> bool {
        let (word, bit) = (i.index() / WORD, 1 << (i.index() % WORD));
        if self.words.len() <= word {
            self.words.resize(word + 1, 0);
        }
        let absent = self.words[word] & bit == 0;
        self.words[word] |= bit;
        absent
    }

    /// Removes `i`, returning whether it was present.
    pub fn remove(&mut self, i: I) -> bool {
        let (word, bit) = (i.index() / WORD, 1 << (i.index() % WORD));
        let Some(w) = self.words.get_mut(word) else { return false };
        let present = *w & bit != 0;
        *w &= !bit;
        present
    }

    #[must_use]
    pub fn contains(&self, i: I) -> bool {
        self.words.get(i.index() / WORD).is_some_and(|w| w & (1 << (i.index() % WORD)) != 0)
    }

    /// Adds every index of `other`, returning whether any was absent.
    pub fn union(&mut self, other: &Self) -> bool {
        if self.words.len() < other.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        let mut changed = false;
        for (w, &o) in self.words.iter_mut().zip(&other.words) {
            changed |= o & !*w != 0;
            *w |= o;
        }
        changed
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn clear(&mut self) {
        self.words.clear();
    }

    /// The indices, ascending.
    pub fn iter(&self) -> impl Iterator<Item = I> + '_ {
        ones(&self.words).map(I::new)
    }

    /// The words without the zeros ending them, which hold nothing.
    fn significant(&self) -> &[u64] {
        let end = self.words.iter().rposition(|&w| w != 0).map_or(0, |at| at + 1);
        &self.words[..end]
    }
}

impl<I: Idx> PartialEq for BitSet<I> {
    fn eq(&self, other: &Self) -> bool {
        self.significant() == other.significant()
    }
}

impl<I: Idx> Eq for BitSet<I> {}

impl<I: Idx + fmt::Debug> fmt::Debug for BitSet<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<I: Idx> FromIterator<I> for BitSet<I> {
    fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}

impl<I: Idx> Extend<I> for BitSet<I> {
    fn extend<T: IntoIterator<Item = I>>(&mut self, iter: T) {
        for i in iter {
            self.insert(i);
        }
    }
}

/// The positions of the bits set in `words`, ascending.
pub(crate) fn ones(words: &[u64]) -> impl Iterator<Item = usize> + '_ {
    words.iter().enumerate().flat_map(|(at, &word)| {
        let mut rest = word;
        std::iter::from_fn(move || {
            (rest != 0).then(|| {
                let bit = rest.trailing_zeros() as usize;
                rest &= rest - 1;
                at * WORD + bit
            })
        })
    })
}
