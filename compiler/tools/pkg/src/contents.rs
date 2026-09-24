//! What an archive holds: the files of a snapshot, read from the directory it was taken of.

use std::io;
use std::path::Path;

use overt_fs::Mode;
use overt_hash::Snapshot;

pub struct Contents<'a> {
    pub dir: &'a Path,
    pub snapshot: &'a Snapshot,
    /// The directory every file is put in, as forges do (`hal-1.0.0`), which fetching strips.
    pub top: &'a str,
}

/// One file to write.
pub struct Packed {
    /// Its path under [`Contents::top`], components joined as in snapshots.
    pub path: String,
    pub bytes: Vec<u8>,
    pub mode: Mode,
}

impl Contents<'_> {
    /// The files in snapshot order, each read from disk.
    pub fn files(&self) -> impl Iterator<Item = io::Result<Packed>> + '_ {
        self.snapshot.files().map(|(components, entry)| {
            let bytes = std::fs::read(components.iter().fold(self.dir.to_path_buf(), |p, c| p.join(c)))?;
            let path = std::iter::once(self.top).chain(components.iter().map(String::as_str));
            let path = path.collect::<Vec<_>>().join(&Snapshot::PATH_SEPARATOR.to_string());
            Ok(Packed { path, bytes, mode: entry.mode })
        })
    }
}
