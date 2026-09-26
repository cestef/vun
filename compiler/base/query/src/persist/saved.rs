//! The saved form: the pools of the handles values hold, then each persisted query's keys and
//! memos. A read is saved as the position of its query and of its key in the file, since slots and
//! tables are the process's own.

use std::fmt;

use vun_codec::{Decode, Encode, Malformed, Pools};

use crate::fingerprint::Fingerprint;

/// The version of the saved form; a file of another version is not read.
pub(super) const FORMAT: u32 = 3;

#[derive(Encode, Decode)]
pub(super) struct Saved {
    /// Leads, so that another version is named rather than misread.
    pub(super) format: u32,
    pub(super) pools: Pools,
    pub(super) queries: Vec<SavedQuery>,
}

/// One query: every key it had a slot for, and its computed memos.
#[derive(Encode, Decode)]
pub(crate) struct SavedQuery {
    pub(super) name: String,
    pub(super) keys: Vec<Vec<u8>>,
    pub(super) memos: Vec<SavedMemo>,
}

#[derive(Encode, Decode)]
pub(crate) struct SavedMemo {
    /// Its position among the query's keys.
    pub(super) key: u32,
    /// `None` for a query persisted without its values.
    pub(super) value: Option<Vec<u8>>,
    pub(super) fingerprint: Fingerprint,
    pub(super) deps: Vec<SavedDep>,
}

/// A read: its query's position in the file, its key's among the query's keys, and the
/// fingerprint it had.
#[derive(Clone, Copy, Encode, Decode)]
pub(crate) struct SavedDep {
    pub(super) query: u32,
    pub(super) key: u32,
    pub(super) fingerprint: Fingerprint,
}

/// Why saved memos could not be loaded.
#[derive(Debug)]
pub enum Unloaded {
    Decode(Malformed),
    /// Written in another version of the saved form.
    Format {
        found: u32,
    },
}

impl fmt::Display for Unloaded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(_) => f.write_str("the saved memos do not decode"),
            Self::Format { found } => write!(f, "the saved memos are in format {found}, not {FORMAT}"),
        }
    }
}

impl std::error::Error for Unloaded {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::Format { .. } => None,
        }
    }
}
