//! Computing queries: the stack of queries being computed, what each reads, and cycles.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use overt_hash::Hash;

use super::Query;
use super::db::{Db, Memo};

/// A query read while another was computed, with the interface hash it had then.
pub(crate) trait Edge: Send + Sync {
    /// Whether the dependency, brought up to date, still has the recorded interface.
    fn unchanged(&self, cx: &Cx<'_>) -> bool;
    fn query(&self) -> TypeId;
    fn key(&self) -> &dyn Any;
    fn hash(&self) -> Hash;
}

pub(crate) struct Read<Q: Query> {
    pub(crate) key: Q::Key,
    pub(crate) hash: Hash,
}

impl<Q: Query> Edge for Read<Q> {
    fn unchanged(&self, cx: &Cx<'_>) -> bool {
        cx.fetch::<Q>(&self.key).1 == self.hash
    }

    fn query(&self) -> TypeId {
        TypeId::of::<Q>()
    }

    fn key(&self) -> &dyn Any {
        &self.key
    }

    fn hash(&self) -> Hash {
        self.hash
    }
}

/// A query being computed.
struct Frame {
    id: (TypeId, Hash),
    name: &'static str,
    key: String,
    reads: Vec<Arc<dyn Edge>>,
}

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

/// The queries being computed on one thread, and what each has read so far.
pub struct Cx<'db> {
    db: &'db Db,
    stack: RefCell<Vec<Frame>>,
}

impl<'db> Cx<'db> {
    #[must_use]
    pub fn new(db: &'db Db) -> Self {
        Self { db, stack: RefCell::new(Vec::new()) }
    }

    #[must_use]
    pub fn db(&self) -> &'db Db {
        self.db
    }

    /// The value of `Q` at `key`, recorded as read by the query being computed.
    pub fn get<Q: Query>(&self, key: &Q::Key) -> Q::Value {
        let (value, hash) = self.fetch::<Q>(key);
        if let Some(top) = self.stack.borrow_mut().last_mut() {
            top.reads.push(Arc::new(Read::<Q> { key: key.clone(), hash }));
        }
        value
    }

    /// The current value of `Q` at `key` and its interface: the memo when it is current or its
    /// trace still holds, otherwise computed again.
    fn fetch<Q: Query>(&self, key: &Q::Key) -> (Q::Value, Hash) {
        let revision = self.db.revision();
        if let Some(memo) = self.db.memo::<Q>(key) {
            if memo.input || memo.verified == Some(revision) {
                return (memo.value, memo.hash);
            }
            if memo.deps.iter().all(|d| d.unchanged(self)) {
                self.db.verified::<Q>(key, revision);
                return (memo.value, memo.hash);
            }
        }
        let id = (TypeId::of::<Q>(), Hash::of_value(key));
        let reached = self.stack.borrow().iter().position(|f| f.id == id);
        if let Some(from) = reached {
            let stack = self.stack.borrow();
            let mut path: Vec<_> = stack[from..].iter().map(|f| (f.name, f.key.clone())).collect();
            path.push((Q::NAME, format!("{key:?}")));
            let value = Q::cycle(self, key, &Cycle { path });
            let hash = Q::interface(&value);
            return (value, hash);
        }
        self.stack.borrow_mut().push(Frame { id, name: Q::NAME, key: format!("{key:?}"), reads: Vec::new() });
        let value = Q::compute(self, key);
        let frame = self.stack.borrow_mut().pop().expect("the frame pushed above");
        let hash = Q::interface(&value);
        let memo = Memo::<Q> {
            value: value.clone(),
            hash,
            deps: frame.reads.into(),
            verified: Some(revision),
            input: false,
        };
        self.db.store(key.clone(), memo);
        (value, hash)
    }
}
