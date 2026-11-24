//! Keys stored with their hash, computed once: a lookup that misses hashes its key once for the
//! lookup and the insertion both, and keys whose hashes differ are never compared.

use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::Arc;

use papaya::Equivalent;

/// A key of a table, with its hash.
pub(crate) struct Hashed<K> {
    hash: u64,
    key: Arc<K>,
}

/// A key looked for, with its hash.
pub(crate) struct Probe<'k, K> {
    pub(crate) hash: u64,
    pub(crate) key: &'k K,
}

impl<'k, K: Hash> Probe<'k, K> {
    pub(crate) fn of(key: &'k K) -> Self {
        Self { hash: foldhash::fast::FixedState::default().hash_one(key), key }
    }
}

impl<K: Clone> Probe<'_, K> {
    /// The key to store, with the hash already known.
    pub(crate) fn stored(&self) -> Hashed<K> {
        self.shared(Arc::new(self.key.clone()))
    }
}

impl<K> Probe<'_, K> {
    /// `key`, equal to the one looked for, stored with its hash: the table holds one copy.
    pub(crate) fn shared(&self, key: Arc<K>) -> Hashed<K> {
        Hashed { hash: self.hash, key }
    }
}

impl<K> Hashed<K> {
    /// The key stored.
    pub(crate) fn key(&self) -> &Arc<K> {
        &self.key
    }
}

/// Replays the hash computed once from the key, which is all a table needs: equality still
/// compares the keys.
impl<K> Hash for Hashed<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

/// The same hash as the [`Hashed`] key it looks for.
impl<K> Hash for Probe<'_, K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl<K: Eq> PartialEq for Hashed<K> {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash && *self.key == *other.key
    }
}

impl<K: Eq> Eq for Hashed<K> {}

impl<K: Eq> Equivalent<Hashed<K>> for Probe<'_, K> {
    fn equivalent(&self, stored: &Hashed<K>) -> bool {
        self.hash == stored.hash && *self.key == *stored.key
    }
}
