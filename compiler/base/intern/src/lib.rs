//! Interned symbols: one pool per process, shared by every thread.
//!
//! Names are compared constantly during resolution and checking, so each distinct string is stored
//! once and handled as a [`Symbol`] afterwards: equality is one pointer comparison. A symbol hashes
//! by the content hash of its text and orders by its text, so every hash and order built from
//! symbols is the same in every process and under every schedule, which persisted query traces
//! and deterministic output need. Interned strings live as long as the process.

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, RwLock};

/// An interned string.
#[derive(Clone, Copy)]
pub struct Symbol(&'static Entry);

struct Entry {
    /// The first bytes of the BLAKE3 hash of the text.
    hash: u64,
    text: &'static str,
}

/// Shards of the pool, chosen by hash, so that threads interning different names rarely wait.
const SHARDS: usize = 16;

type Shard = RwLock<HashMap<&'static str, Symbol>>;

static POOL: LazyLock<[Shard; SHARDS]> = LazyLock::new(|| std::array::from_fn(|_| RwLock::default()));

impl Symbol {
    /// The symbol of `text`, the same for every call with equal text.
    ///
    /// # Panics
    /// Panics if a thread panicked while interning.
    #[must_use]
    pub fn intern(text: &str) -> Self {
        let hash = overt_hash::Hash::of(text.as_bytes()).short();
        let shard = &POOL[usize::try_from(hash % SHARDS as u64).expect("fits")];
        if let Some(&symbol) = shard.read().expect("interner").get(text) {
            return symbol;
        }
        let mut shard = shard.write().expect("interner");
        // Another thread may have interned it between the two locks.
        if let Some(&symbol) = shard.get(text) {
            return symbol;
        }
        let text: &'static str = Box::leak(text.into());
        let symbol = Self(Box::leak(Box::new(Entry { hash, text })));
        shard.insert(text, symbol);
        symbol
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.0.text
    }
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for Symbol {}

impl Hash for Symbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.hash);
    }
}

impl PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Symbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests;
