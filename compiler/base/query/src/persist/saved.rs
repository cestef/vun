//! The saved form: a header, then the body: the pools of the handles values hold, then each
//! persisted query's keys and memos. A read is saved as the position of its query and of its key
//! in the file, since slots and tables are the process's own. Written with bytes, `B` is read as
//! [`Blob`]s: keys, values and pool entries are places in the file loaded, never copied out of it.
//!
//! A body extends the one loaded, whose pool entries it keeps whether still referenced or not, so
//! it is written whole again once it outgrows its size when last written whole.

use std::fmt;

use vun_codec::{Blob, Decode, Encode, Malformed, Pools};

use crate::fingerprint::Fingerprint;

/// The version of the saved form; a file of another version is not read.
pub(super) const FORMAT: u32 = 7;

/// How many times its size when last written whole a body grows to before it is written whole.
const GROWTH: u64 = 2;

#[derive(Encode, Decode)]
pub(crate) struct Saved<B = Vec<u8>> {
    /// Leads, so that another version is named rather than misread.
    pub(crate) format: u32,
    /// The size of the body when last written whole.
    pub(crate) compacted: u64,
    pub(crate) body: B,
}

/// The pools, then each query encoded on its own, so that queries are read in parallel.
#[derive(Encode, Decode)]
pub(super) struct Body<B = Vec<u8>> {
    pub(super) pools: Pools<B>,
    pub(super) queries: Vec<B>,
}

/// The body as read, in place.
pub(super) type Loaded = Body<Blob>;

/// The size of the body loaded, and its size when last written whole.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Growth {
    pub(super) size: u64,
    pub(super) compacted: u64,
}

impl Growth {
    /// Whether the next body is written whole rather than extending this one.
    pub(super) fn outgrown(self) -> bool {
        self.size > GROWTH * self.compacted
    }
}

/// One query: every key it had a slot for, and its computed memos.
#[derive(Encode, Decode)]
pub(crate) struct SavedQuery<B = Vec<u8>> {
    pub(super) name: String,
    pub(super) keys: Vec<SavedKey<B>>,
    pub(super) memos: Vec<SavedMemo<B>>,
}

/// A key, and the fingerprint of its memo if it had one: its saved memo's, and the one reads of
/// it saw unless they record their own.
#[derive(Encode, Decode)]
pub(crate) struct SavedKey<B = Vec<u8>> {
    pub(super) key: B,
    pub(super) fingerprint: Option<Fingerprint>,
}

#[derive(Encode, Decode)]
pub(crate) struct SavedMemo<B = Vec<u8>> {
    /// Its position among the query's keys.
    pub(super) key: u32,
    /// `None` for a query persisted without its values.
    pub(super) value: Option<B>,
    pub(super) deps: Vec<SavedDep>,
}

/// A read: its query's position in the file, its key's among the query's keys, and the
/// fingerprint it had when that is not its key's: most reads saw the memo saved, and sixteen
/// bytes a read would be most of the file.
#[derive(Clone, Copy, Encode, Decode)]
pub(crate) struct SavedDep {
    pub(super) query: u32,
    pub(super) key: u32,
    pub(super) fingerprint: Option<Fingerprint>,
}

/// Why saved memos could not be loaded.
#[derive(Debug)]
pub enum Unloaded {
    Decode(Malformed),
    /// Written in another version of the saved form.
    Format {
        found: u32,
    },
    /// The database holds memos loaded already.
    Again,
}

impl fmt::Display for Unloaded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(_) => f.write_str("the saved memos do not decode"),
            Self::Format { found } => write!(f, "the saved memos are in format {found}, not {FORMAT}"),
            Self::Again => f.write_str("memos were loaded already"),
        }
    }
}

impl std::error::Error for Unloaded {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::Format { .. } | Self::Again => None,
        }
    }
}
