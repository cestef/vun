//! Interned symbols.
//!
//! Names are compared constantly during resolution and checking, so they are interned once and
//! handled as [`Symbol`] indices afterwards.

use std::collections::HashMap;

use overt_index::{IndexVec, index};

index! {
    pub struct Symbol;
}

#[derive(Default)]
pub struct Interner {
    ids: HashMap<Box<str>, Symbol>,
    strings: IndexVec<Symbol, Box<str>>,
}

impl Interner {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns a string, returning the existing symbol if it was seen before.
    pub fn intern(&mut self, s: &str) -> Symbol {
        if let Some(&sym) = self.ids.get(s) {
            return sym;
        }
        let sym = self.strings.push(s.into());
        self.ids.insert(s.into(), sym);
        sym
    }

    #[must_use]
    pub fn resolve(&self, sym: Symbol) -> &str {
        &self.strings[sym]
    }
}

#[cfg(test)]
mod tests;
