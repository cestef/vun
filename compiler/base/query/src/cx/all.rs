//! Many keys of one query at once: computed over threads, each on a context of its own, then
//! read in order through the caller's, which records them as read.

use vun_par::Parallel;

use super::Cx;
use crate::Query;

impl Cx<'_> {
    /// The values of `Q` at `keys`, in order, recorded as read. Those not computed yet are
    /// computed over threads first: each is a query of its own, so what it reads is recorded
    /// in its own memo, and the caller's reads of it are what verify the caller.
    pub fn get_all<Q: Query>(&self, keys: &[Q::Key]) -> Vec<Q::Value> {
        let db = self.db;
        keys.par_map(|key| drop(Cx::new(db).get::<Q>(key)));
        keys.iter().map(|key| self.get::<Q>(key)).collect()
    }
}
