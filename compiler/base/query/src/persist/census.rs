//! What a saved file holds, counted per query and per pool without loading it into a database:
//! what `vun debug queries` shows.

use vun_codec::{Blob, Malformed};

use super::Unloaded;
use super::saved::{Loaded, Saved, SavedDep, SavedMemo, SavedQuery};

/// The counts of a saved file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Census {
    pub bytes: usize,
    /// The size of the body when last written whole.
    pub compacted: u64,
    pub queries: Vec<Counted>,
    pub pools: Vec<Counted>,
}

/// One query's keys, memos and reads, or one pool's entries, with the bytes of their contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Counted {
    pub name: String,
    /// Keys of a query, entries of a pool.
    pub entries: usize,
    pub memos: usize,
    pub reads: usize,
    /// Bytes of the keys and values of a query, of the entries of a pool.
    pub bytes: usize,
}

impl Census {
    /// # Errors
    /// Returns an error if the bytes are not saved memos of this format.
    pub fn of(bytes: &[u8]) -> Result<Self, Unloaded> {
        let header = Saved::header(bytes)?;
        let body: Loaded = header.body.read(bytes).map_err(Unloaded::Decode)?;
        let sum = |blobs: &mut dyn Iterator<Item = Blob>| blobs.map(|b| b.len).sum::<usize>();
        let queries = body.queries.iter().map(|q| {
            let SavedQuery { keys, replaced } = q.query.read::<SavedQuery<Blob>>(bytes)?;
            let keys = keys.read(bytes)?;
            let memos = keys.iter().filter_map(|k| k.memo).chain(replaced.iter().map(|r| r.memo));
            let memos = memos.map(|m| m.read::<SavedMemo<Blob>>(bytes));
            let memos = memos.collect::<Result<Vec<_>, _>>()?;
            let reads = memos.iter().map(|m| Ok(m.deps.read::<Vec<SavedDep>>(bytes)?.len()));
            let reads = reads.sum::<Result<usize, Malformed>>()?;
            Ok(Counted {
                name: q.name.clone(),
                entries: keys.len(),
                memos: memos.len(),
                reads,
                bytes: sum(&mut keys.iter().map(|k| k.key)) + sum(&mut memos.iter().filter_map(|m| m.value)),
            })
        });
        let queries = queries.collect::<Result<Vec<_>, _>>().map_err(Unloaded::Decode)?;
        let pools = body.pools.0.iter().map(|p| Counted {
            name: p.name.clone(),
            entries: p.entries.len(),
            memos: 0,
            reads: 0,
            bytes: sum(&mut p.entries.iter().copied()),
        });
        Ok(Self { bytes: bytes.len(), compacted: header.compacted, queries, pools: pools.collect() })
    }
}
