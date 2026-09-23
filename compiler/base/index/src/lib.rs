//! Typed indices and the vectors they index.
//!
//! Every arena in the compiler (files, symbols, origins, IR nodes) is an [`IndexVec`] addressed by
//! its own `u32` newtype, declared with [`index!`], so indices of different tables cannot be mixed.

use std::marker::PhantomData;
use std::ops;

/// A `u32` newtype usable as an index.
pub trait Idx: Copy + Eq {
    /// # Panics
    /// Panics if `i` does not fit in `u32`.
    fn new(i: usize) -> Self;
    fn index(self) -> usize;
}

/// Declares `u32` index newtypes implementing [`Idx`].
#[macro_export]
macro_rules! index {
    ($($(#[$attr:meta])* $vis:vis struct $name:ident;)*) => {$(
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        $vis struct $name(u32);

        impl $crate::Idx for $name {
            fn new(i: usize) -> Self {
                Self(u32::try_from(i).expect(concat!(stringify!($name), " overflowed u32")))
            }

            fn index(self) -> usize {
                self.0 as usize
            }
        }
    )*};
}

/// A vector indexed by `I` instead of `usize`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexVec<I: Idx, T> {
    raw: Vec<T>,
    _index: PhantomData<fn(&I)>,
}

impl<I: Idx, T> Default for IndexVec<I, T> {
    fn default() -> Self {
        Self { raw: Vec::new(), _index: PhantomData }
    }
}

impl<I: Idx, T> IndexVec<I, T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self { raw: Vec::with_capacity(capacity), _index: PhantomData }
    }

    /// Appends `value` and returns its index.
    pub fn push(&mut self, value: T) -> I {
        let i = I::new(self.raw.len());
        self.raw.push(value);
        i
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    #[must_use]
    pub fn get(&self, i: I) -> Option<&T> {
        self.raw.get(i.index())
    }

    #[must_use]
    pub fn last(&self) -> Option<&T> {
        self.raw.last()
    }

    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.raw.iter().enumerate().map(|(i, v)| (I::new(i), v))
    }
}

impl<I: Idx, T> ops::Index<I> for IndexVec<I, T> {
    type Output = T;

    fn index(&self, i: I) -> &T {
        &self.raw[i.index()]
    }
}

impl<I: Idx, T> ops::IndexMut<I> for IndexVec<I, T> {
    fn index_mut(&mut self, i: I) -> &mut T {
        &mut self.raw[i.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    index! {
        struct Node;
    }

    #[test]
    fn push_returns_consecutive_indices() {
        let mut v: IndexVec<Node, &str> = IndexVec::new();
        let a = v.push("a");
        let b = v.push("b");
        assert_eq!((a.index(), b.index()), (0, 1));
        assert_eq!(v[b], "b");
        assert_eq!(v.iter().map(|(i, _)| i).collect::<Vec<_>>(), [a, b]);
    }
}
