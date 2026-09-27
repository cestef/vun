//! What an archive holds: the files of a snapshot, with the bytes it was hashed from.

use vun_fs::Mode;
use vun_hash::{Snapshot, Taken};

pub struct Contents<'a> {
    pub taken: &'a Taken,
    /// The directory every file is put in, as forges do (`hal-1.0.0`), which fetching strips.
    pub top: &'a str,
}

/// One file to write.
pub struct Packed<'a> {
    /// Its path under [`Contents::top`], components joined as in snapshots.
    pub path: String,
    pub bytes: &'a [u8],
    pub mode: Mode,
}

impl Contents<'_> {
    /// The files in snapshot order.
    pub fn files(&self) -> impl Iterator<Item = Packed<'_>> + '_ {
        self.taken.files().map(|(components, entry, bytes)| {
            let path = std::iter::once(self.top).chain(components.iter().map(String::as_str));
            let path = path.collect::<Vec<_>>().join(&Snapshot::PATH_SEPARATOR.to_string());
            Packed { path, bytes, mode: entry.mode }
        })
    }
}
