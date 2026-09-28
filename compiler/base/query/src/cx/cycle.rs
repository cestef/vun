//! Queries that depend on themselves: found on the path of queries being computed, this
//! context's own and those of the contexts that lent it its work, so that a cycle through
//! `get_all` is one as it is on one thread.

use std::fmt;

use vun_trace::tracing;

use super::Cx;
use crate::table::{Slot, TableId};

/// Queries that depend on themselves: each by name and key, from the one reached again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle {
    pub path: Vec<(&'static str, String)>,
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("query cycle: ")?;
        for (i, (name, key)) in self.path.iter().enumerate() {
            if i > 0 {
                f.write_str(" then ")?;
            }
            write!(f, "{name}({key})")?;
        }
        Ok(())
    }
}

impl std::error::Error for Cycle {}

impl Cx<'_> {
    /// What the path reaches again by computing `slot` of `table`, if it computes it already.
    /// What does not depend on the query is kept out of the code made for each one.
    #[inline(never)]
    pub(super) fn cycle(&self, table: TableId, slot: Slot) -> Option<Cycle> {
        let own = self.computing.borrow().get(&(table, slot)).copied();
        let from = match own {
            Some(at) => self.above.len() + at,
            None => self.above.iter().position(|&frame| frame == (table, slot))?,
        };
        self.reach(from);
        let stack = self.stack.borrow();
        let own = stack.iter().map(|frame| (frame.table, frame.slot));
        let path = self.above.iter().copied().chain(own).skip(from).chain([(table, slot)]);
        let path = path.map(|(table, slot)| {
            let table = self.db.any_table(table);
            (table.name(), table.show(slot))
        });
        let cycle = Cycle { path: path.collect() };
        tracing::debug!(%cycle, "cycle recovered");
        Some(cycle)
    }

    /// Records that the computation on top of the stack, or the lender's when the stack is
    /// empty, read the cycle value of the frame at `head` on the path.
    pub(super) fn reach(&self, head: usize) {
        let lower = |at: Option<usize>| Some(at.map_or(head, |at| at.min(head)));
        match self.stack.borrow_mut().last_mut() {
            Some(top) => top.head = lower(top.head),
            None => self.reached.set(lower(self.reached.get())),
        }
    }

    /// The path of queries being computed, this context's frames last: what the contexts it
    /// lends work to compute above.
    pub(super) fn path(&self) -> std::sync::Arc<[(TableId, Slot)]> {
        let stack = self.stack.borrow();
        self.above.iter().copied().chain(stack.iter().map(|frame| (frame.table, frame.slot))).collect()
    }
}
