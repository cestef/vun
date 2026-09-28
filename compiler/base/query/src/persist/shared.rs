//! Memos other databases start from: those whose every read leads to inputs set with
//! [`Db::share`] alone, such as the libraries every project builds on, saved apart so that a
//! database holding none of its own loads them instead of computing them again.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;

use vun_trace::tracing;

use super::super::db::Db;
use super::load::Taking;
use crate::table::{Slot, TableId};

/// The keys a file of shared memos holds, by table and slot.
pub(crate) type Included = HashSet<(TableId, Slot)>;

impl Db {
    /// The memos of persisted queries whose every read leads to shared inputs alone, with those
    /// inputs, saved whole; `None` when this process computed none of them since they were last
    /// saved, so that the ones loaded or saved are what a save would write. A database that
    /// loaded no memos takes these as the shared memos it is layered over, so that the memos it
    /// saves next hold its own alone.
    ///
    /// # Panics
    /// If the registry was poisoned by a panicking query.
    #[must_use]
    pub fn save_shared(&self) -> Option<Vec<u8>> {
        // Nothing computed: what was loaded is what a save would write.
        if !self.unshared.swap(false, Ordering::Relaxed) {
            return None;
        }
        let persisted = self.persisted.read().expect("persisted queries").clone();
        let tables: HashSet<TableId> = persisted.iter().map(|p| p.table(self)).collect();
        let mut sharing = Sharing { db: self, tables: &tables, known: HashMap::new(), computed: false };
        // The memos this process computed have slots: whether one is shared is known before the
        // memos loaded and never asked for are given theirs, which a save writes too.
        sharing.all();
        if !sharing.computed {
            return None;
        }
        if let Some(store) = self.store.get() {
            store.restore_all(self, false);
            sharing.all();
        }
        let included: Included =
            sharing.known.into_iter().filter(|&(_, shared)| shared).map(|(k, _)| k).collect();
        let bytes = self.save_keys(Some(&included));
        if !self.restored()
            && let Err(unloaded) = self.layer(bytes.clone(), None, Taking::Written)
        {
            tracing::debug!(%unloaded, "the shared memos saved are not layered under");
        }
        Some(bytes)
    }
}

/// Which memos are shared, found once each.
struct Sharing<'a> {
    db: &'a Db,
    /// The persisted tables: a read of any other is never saved.
    tables: &'a HashSet<TableId>,
    known: HashMap<(TableId, Slot), bool>,
    /// Whether a shared memo was computed in this process.
    computed: bool,
}

impl Sharing<'_> {
    /// Finds whether each memo of the persisted tables is shared.
    fn all(&mut self) {
        for &table in self.tables {
            for slot in (0..self.db.any_table(table).len()).map(Slot::at) {
                self.shared((table, slot));
            }
        }
    }

    /// Whether the memo at `key` is shared: a shared input, or a memo whose reads all are. A read
    /// reached again while its own reads are being looked at counts as not shared.
    fn shared(&mut self, key: (TableId, Slot)) -> bool {
        if let Some(&shared) = self.known.get(&key) {
            return shared;
        }
        self.known.insert(key, false);
        // Each memo with the reads left to look at.
        let mut stack = vec![(key, self.facts(key))];
        while let Some((at, facts)) = stack.last_mut() {
            let at = *at;
            let Some(facts) = facts else {
                stack.pop();
                continue;
            };
            let Some(dep) = facts.deps.pop() else {
                self.known.insert(at, true);
                if facts.computed {
                    let table = self.db.any_table(at.0);
                    tracing::debug!(query = table.name(), key = table.show(at.1), "a shared memo computed");
                }
                self.computed |= facts.computed;
                stack.pop();
                continue;
            };
            let next = (dep.table, dep.slot);
            match self.known.get(&next) {
                Some(true) => {}
                Some(false) => {
                    stack.pop();
                }
                // Looked at again once its own reads are known.
                None => {
                    facts.deps.push(dep);
                    self.known.insert(next, false);
                    let facts = self.facts(next);
                    stack.push((next, facts));
                }
            }
        }
        self.known[&key]
    }

    /// What the memo at `key` read, if it can be shared by what it is: a shared input's reads are
    /// none, and a memo of a query not persisted, or an input not shared, cannot.
    fn facts(&self, (table, slot): (TableId, Slot)) -> Option<Remaining> {
        if !self.tables.contains(&table) {
            return None;
        }
        let facts = self.db.any_table(table).facts(self.db, slot)?;
        match (facts.input, facts.shared) {
            (true, false) => None,
            (true, true) => Some(Remaining { deps: Vec::new(), computed: false }),
            (false, _) => Some(Remaining { deps: facts.deps.into_vec(), computed: facts.computed }),
        }
    }
}

/// A memo being looked at: the reads left, and whether it was computed in this process.
struct Remaining {
    deps: Vec<crate::memo::Dep>,
    computed: bool,
}
