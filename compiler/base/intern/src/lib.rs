//! Interned symbols: one pool per process, shared by every thread.
//!
//! Names are compared constantly during resolution and checking, so each distinct string is stored
//! once and handled as a [`Symbol`] afterwards: equality is one pointer comparison. A symbol hashes
//! by the content hash of its text and orders by its text, so every hash and order built from
//! symbols is the same in every process and under every schedule, which persisted query traces
//! and deterministic output need. Interned strings live as long as the process.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

use vun_par::Map;

/// An interned string.
#[derive(Clone, Copy)]
pub struct Symbol(&'static Entry);

struct Entry {
    /// The first bytes of the BLAKE3 hash of the text.
    hash: u64,
    text: &'static str,
}

mod codec;
mod pool;

pub use codec::Internable;
pub use pool::{Interned, Pool};

/// Every symbol by its text, looked up without locks by a fast hash.
static POOL: LazyLock<Map<&'static str, Symbol>> = LazyLock::new(Map::default);

impl Symbol {
    /// The symbol of `text`, the same for every call with equal text; its content hash is
    /// computed once, when the text is first interned.
    #[must_use]
    pub fn intern(text: &str) -> Self {
        let symbols = POOL.pin();
        if let Some(&symbol) = symbols.get(text) {
            return symbol;
        }
        let hash = vun_hash::Hash::of(text.as_bytes()).short();
        let text: &'static str = Box::leak(text.into());
        let symbol = Self(Box::leak(Box::new(Entry { hash, text })));
        // Another thread may intern the same text meanwhile: the first stored stands for both.
        match symbols.try_insert(text, symbol) {
            Ok(&symbol) => symbol,
            Err(occupied) => *occupied.current,
        }
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
