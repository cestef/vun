//! Interned values of any type, with the properties of [`crate::Symbol`]: stored once per process,
//! equal by pointer, hashed by the stable hash of the value. A value's `Hash` must depend on its
//! content only (symbols, integers and other interned values qualify), so that the stable hash,
//! and every hash built from interned values, is the same in every process.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::LazyLock;

use vun_par::Map;

/// The pool of one type's interned values, declared as a `static` by the type that uses it:
/// looked up without locks, by a fast hash; the stable hash is computed once, when a value is
/// first interned.
pub struct Pool<T: 'static> {
    values: LazyLock<Map<&'static T, Interned<T>>>,
}

/// A value stored in a [`Pool`].
pub struct Interned<T: 'static>(&'static Entry<T>);

struct Entry<T> {
    /// The [`vun_hash::Stable`] hash of the value.
    hash: u64,
    value: T,
}

impl<T: Hash + Eq + Send + Sync + 'static> Pool<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self { values: LazyLock::new(Map::default) }
    }

    /// The interned `value`, the same for every call with an equal value.
    pub fn intern(&'static self, value: T) -> Interned<T> {
        let values = self.values.pin();
        if let Some(&interned) = values.get(&value) {
            return interned;
        }
        let hash = vun_hash::Stable::of(&value);
        let entry: &'static Entry<T> = Box::leak(Box::new(Entry { hash, value }));
        // Another thread may intern an equal value meanwhile: the first stored stands for both.
        match values.try_insert(&entry.value, Interned(entry)) {
            Ok(&interned) => interned,
            Err(occupied) => *occupied.current,
        }
    }
}

impl<T: Hash + Eq + Send + Sync + 'static> Default for Pool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Interned<T> {
    /// The value, stored for the rest of the process.
    #[must_use]
    pub fn get(self) -> &'static T {
        &self.0.value
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

/// The stable hash of the value, computed once when it was interned: equal values are one entry.
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
