//! Interned values of any type, with the properties of [`crate::Symbol`]: stored once per process,
//! equal by pointer, hashed by the content hash of the value. A value's `Hash` must depend on its
//! content only (symbols, integers and other interned values qualify), so that the content hash,
//! and every hash built from interned values, is the same in every process.

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{LazyLock, RwLock};

use crate::SHARDS;

type Shard<T> = RwLock<HashMap<&'static T, Interned<T>>>;

/// The pool of one type's interned values, declared as a `static` by the type that uses it.
pub struct Pool<T: 'static> {
    shards: LazyLock<[Shard<T>; SHARDS]>,
}

/// A value stored in a [`Pool`].
pub struct Interned<T: 'static>(&'static Entry<T>);

struct Entry<T> {
    /// The first bytes of the BLAKE3 hash of the value.
    hash: u64,
    value: T,
}

impl<T: Hash + Eq + Send + Sync + 'static> Pool<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self { shards: LazyLock::new(Self::shards) }
    }

    fn shards() -> [Shard<T>; SHARDS] {
        std::array::from_fn(|_| RwLock::default())
    }

    /// The interned `value`, the same for every call with an equal value.
    ///
    /// # Panics
    /// Panics if a thread panicked while interning.
    pub fn intern(&'static self, value: T) -> Interned<T> {
        let hash = overt_hash::Hash::of_value(&value).short();
        let shard = &self.shards[usize::try_from(hash % SHARDS as u64).expect("fits")];
        if let Some(&interned) = shard.read().expect("pool").get(&value) {
            return interned;
        }
        let mut shard = shard.write().expect("pool");
        // Another thread may have interned it between the two locks.
        if let Some(&interned) = shard.get(&value) {
            return interned;
        }
        let entry: &'static Entry<T> = Box::leak(Box::new(Entry { hash, value }));
        shard.insert(&entry.value, Interned(entry));
        Interned(entry)
    }
}

impl<T: Hash + Eq + Send + Sync + 'static> Default for Pool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for Interned<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Interned<T> {}

impl<T> Deref for Interned<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0.value
    }
}

impl<T> PartialEq for Interned<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl<T> Eq for Interned<T> {}

impl<T> Hash for Interned<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.hash);
    }
}

impl<T: fmt::Debug> fmt::Debug for Interned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.value.fmt(f)
    }
}

impl<T: fmt::Display> fmt::Display for Interned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.value.fmt(f)
    }
}
