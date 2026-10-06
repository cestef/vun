//! The saved form: a header, then the body: the pools of the handles values hold, then each
//! persisted query's keys, each with its memo. A read is saved as the position of its query and
//! of its key in the file, since slots and tables are the process's own. Written with bytes, `B`
//! is read as [`Blob`]s: queries, keys, memos, values and pool entries are places in the file
//! loaded, never copied out of it, and each is read when first needed: a key by the stable hash
//! of its value, a memo when its key is.
//!
//! A body extends the one loaded, keeping every position in it and every entry it holds whether
//! still referenced or not, so it is written whole again once it outgrows its size when last
//! written whole.
//!
//! A file can be layered over a file of shared memos, named by the hash of its body: its stream
//! follows that file's, its pools number their entries after that file's, each query of that
//! file is at the same position in it, and each query's keys follow that file's keys of the
//! query, so that the positions that file's memos read by mean the same in both. It holds the
//! keys that file lacks, and the memos of that file's keys it computed again.

use std::fmt;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use vun_codec::{Blob, Context, Decode, Encode, Malformed, Pools};
use vun_hash::Hash;

use crate::db::Sharing;
use crate::fingerprint::Fingerprint;

/// The version of the saved form; a file of another version is not read.
pub(super) const FORMAT: u32 = 12;

/// The compiler running, as saved memos name it.
pub struct Build;

impl Build {
    /// The running executable ([`vun_fs::EXECUTABLE`]), by its path, its size and when it was
    /// written: another build of the compiler is another executable, and its memos may hold
    /// values of types changed since. `None` when the executable cannot be found, where no memo
    /// is trusted.
    #[must_use]
    pub fn identity() -> Option<Hash> {
        static IDENTITY: OnceLock<Option<Hash>> = OnceLock::new();
        *IDENTITY.get_or_init(|| {
            let exe = vun_fs::EXECUTABLE.as_deref()?;
            let meta = std::fs::metadata(exe).ok()?;
            let written = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
            Some(Hash::of_value(&(exe.as_os_str().as_encoded_bytes(), meta.len(), written.as_nanos())))
        })
    }
}

/// How many times its size when last written whole a body grows to before it is written whole.
const GROWTH: u64 = 2;

#[derive(Encode, Decode)]
pub(crate) struct Saved<B = Vec<u8>> {
    /// Leads, so that another version is named rather than misread.
    pub(crate) format: u32,
    /// The compiler that wrote it ([`Build::identity`]), whose types its values are of.
    pub(crate) build: Option<Hash>,
    /// The size of the body when last written whole.
    pub(crate) compacted: u64,
    /// The hash of the body of a file of shared memos, which files layered over it name it by.
    pub(crate) id: Option<Hash>,
    /// The file of shared memos it is layered over.
    pub(crate) base: Option<Hash>,
    pub(crate) body: B,
}

impl Saved<Blob> {
    /// The header of `bytes`, if they are saved memos of this format.
    pub(super) fn header(bytes: &[u8]) -> Result<Self, Unloaded> {
        let format = Context::default().front::<u32>(bytes).map_err(Unloaded::Decode)?;
        if format != FORMAT {
            return Err(Unloaded::Format { found: format });
        }
        Self::decoded(bytes).map_err(Unloaded::Decode)
    }
}

/// What a saved file is to others: its name as a file of shared memos, and the file of shared
/// memos it is layered over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layering {
    pub id: Option<Hash>,
    pub base: Option<Hash>,
}

impl Layering {
    /// # Errors
    /// Returns an error if the bytes are not saved memos of this format.
    pub fn of(bytes: &[u8]) -> Result<Self, Unloaded> {
        let header = Saved::header(bytes)?;
        Ok(Self { id: header.id, base: header.base })
    }
}

/// The pools, then each query by name, encoded on its own so that it is read when first needed.
#[derive(Encode, Decode)]
pub(super) struct Body<B = Vec<u8>> {
    pub(super) pools: Pools<B>,
    pub(super) queries: Vec<Named<B>>,
}

#[derive(Encode, Decode)]
pub(super) struct Named<B = Vec<u8>> {
    pub(super) name: String,
    /// A [`SavedQuery`].
    pub(super) query: B,
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

/// One query: every key it had a slot for, each with its memo if it was computed, and in a file
/// layered over another, the memos of that file's keys computed again.
#[derive(Encode, Decode)]
pub(crate) struct SavedQuery<B = Vec<u8>> {
    pub(super) keys: Vec<SavedKey<B>>,
    pub(super) replaced: Vec<Replaced<B>>,
}

/// The memo of a key of the file layered under, at `at` among its keys: a [`SavedMemo`].
#[derive(Encode, Decode)]
pub(crate) struct Replaced<B = Vec<u8>> {
    pub(super) at: u32,
    pub(super) memo: B,
}

/// A key, the stable hash of its value, by which it is found, and the fingerprint of its memo if
/// it had one when first saved: the one its memo and reads of it saw unless they record their
/// own.
#[derive(Clone, Encode, Decode)]
pub(crate) struct SavedKey<B = Vec<u8>> {
    pub(super) key: B,
    pub(super) hash: u64,
    pub(super) fingerprint: Option<Fingerprint>,
    /// A [`SavedMemo`].
    pub(super) memo: Option<B>,
}

#[derive(Encode, Decode)]
pub(crate) struct SavedMemo<B = Vec<u8>> {
    /// Its fingerprint when it is not its key's.
    pub(super) fingerprint: Option<Fingerprint>,
    /// `None` for a query persisted without its values.
    pub(super) value: Option<B>,
    /// The reads, a list of [`SavedDep`]s, left unread until the memo is verified.
    pub(super) deps: B,
    /// For a memo whose inputs are shared, how far, and their fingerprint when it last held.
    pub(super) under: Option<(Sharing, Fingerprint)>,
}

/// A read: its query's position in the file, its key's among the query's keys, and the
/// fingerprint it had when that is not its key's: most reads saw the memo saved, and sixteen
/// bytes a read would be most of the file.
#[derive(Clone, Copy, Debug, Encode, Decode)]
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
    /// Written by another build of the compiler, whose saved types may differ from this one's.
    Build,
    /// Layered over a file of shared memos other than the one given, or given one while layered
    /// over none.
    Base,
}

impl fmt::Display for Unloaded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(_) => f.write_str("the saved memos do not decode"),
            Self::Format { found } => write!(f, "the saved memos are in format {found}, not {FORMAT}"),
            Self::Again => f.write_str("memos were loaded already"),
            Self::Build => f.write_str("the saved memos were written by another build of the compiler"),
            Self::Base => f.write_str("the saved memos are layered over other shared memos"),
        }
    }
}

impl std::error::Error for Unloaded {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::Format { .. } | Self::Again | Self::Build | Self::Base => None,
        }
    }
}
