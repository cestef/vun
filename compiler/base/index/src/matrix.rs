//! Sets of columns for each of a fixed number of rows, as one block of bits: a row per `R`, its
//! words side by side. Copying or joining every row is one pass over the words, as a flow over
//! a function's locals needs at each block.

use std::marker::PhantomData;

use crate::bits::{WORD, ones};
use crate::{BitSet, Idx};

#[derive(Clone, PartialEq, Eq)]
pub struct BitMatrix<R: Idx, C: Idx> {
    /// Words per row.
    stride: usize,
    words: Vec<u64>,
    _index: PhantomData<fn(&R, &C)>,
}

impl<R: Idx, C: Idx> BitMatrix<R, C> {
    /// `rows` empty rows of `columns` columns.
    #[must_use]
    pub fn new(rows: usize, columns: usize) -> Self {
        let stride = columns.div_ceil(WORD);
        Self { stride, words: vec![0; rows * stride], _index: PhantomData }
    }

    fn row(&self, r: R) -> &[u64] {
        &self.words[r.index() * self.stride..][..self.stride]
    }

    fn row_mut(&mut self, r: R) -> &mut [u64] {
        &mut self.words[r.index() * self.stride..][..self.stride]
    }

    /// Adds `c` to row `r`, returning whether it was absent.
    pub fn insert(&mut self, r: R, c: C) -> bool {
        let word = &mut self.row_mut(r)[c.index() / WORD];
        let bit = 1 << (c.index() % WORD);
        let absent = *word & bit == 0;
        *word |= bit;
        absent
    }

    #[must_use]
    pub fn contains(&self, r: R, c: C) -> bool {
        self.row(r)[c.index() / WORD] & (1 << (c.index() % WORD)) != 0
    }

    /// Adds every column of every row of `other`, returning whether any was absent.
    pub fn union(&mut self, other: &Self) -> bool {
        let mut changed = false;
        for (w, &o) in self.words.iter_mut().zip(&other.words) {
            changed |= o & !*w != 0;
            *w |= o;
        }
        changed
    }

    /// Makes row `r` hold `set`, which holds no column past the matrix's.
    pub fn set_row(&mut self, r: R, set: &BitSet<C>) {
        self.row_mut(r).fill(0);
        self.union_row(r, set);
    }

    /// Adds `set`, which holds no column past the matrix's, to row `r`.
    pub fn union_row(&mut self, r: R, set: &BitSet<C>) {
        for c in set.iter() {
            self.insert(r, c);
        }
    }

    /// The columns of row `r`, ascending.
    pub fn iter_row(&self, r: R) -> impl Iterator<Item = C> + '_ {
        ones(self.row(r)).map(C::new)
    }
}
