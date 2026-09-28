//! Many keys of one query at once: computed over threads, each on a context of its own under the
//! caller's path, then read in order as the caller's reads.

use vun_par::Parallel;

use super::Cx;
use crate::Query;
use crate::memo::Dep;

impl Cx<'_> {
    /// The values of `Q` at `keys`, in order, recorded as read. Those not computed yet are
    /// computed over threads first: each is a query of its own, so what it reads is recorded
    /// in its own memo, and the caller's reads of it are what verify the caller. A cycle one of
    /// them closes through the caller's path is the caller's, as it is on one thread.
    pub fn get_all<Q: Query>(&self, keys: &[Q::Key]) -> Vec<Q::Value> {
        let (db, above) = (self.db, self.path());
        let table = db.table::<Q>();
        let found = keys.par_map(|key| {
            let cx = Cx::under(db, std::sync::Arc::clone(&above));
            let slot = table.slot(key);
            let (value, fingerprint) = cx.fetch(table, slot);
            (Dep { table: table.id, slot, fingerprint }, value, cx.reached.get())
        });
        let read = found.into_iter().map(|(read, value, reached)| {
            self.record(read);
            if let Some(head) = reached {
                self.reach(head);
            }
            value
        });
        read.collect()
    }
}
