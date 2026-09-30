//! Where a fetch builds an entry before moving it in place: a directory of its own process under
//! `staging/`, held while it exists so collection waits for it, and removed when dropped.

use std::fs::{File, OpenOptions};
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use vun_fs::Edit;
use vun_hash::Hash;

use super::Store;

pub struct Staging {
    dir: PathBuf,
    /// The store's lock, held shared: collection takes it alone.
    _held: File,
}

impl Store {
    /// The file processes lock: shared while staging, alone while collecting.
    pub const LOCK: &str = "lock";

    /// A fresh staging directory for an entry of `hash`, this process's own.
    ///
    /// # Errors
    /// Returns an error if the store cannot be locked or the directory made.
    pub fn stage(&self, hash: Hash) -> io::Result<Staging> {
        let held = self.lock()?;
        held.lock_shared()?;
        let dir = self.root.join(Self::STAGING).join(hash.algorithm().name()).join(hash.digest());
        let dir = dir.join(vun_fs::PROCESS.to_string());
        dir.fresh()?;
        Ok(Staging { dir, _held: held })
    }

    /// The store's lock file, opened, not yet locked.
    pub(super) fn lock(&self) -> io::Result<File> {
        std::fs::create_dir_all(&self.root)?;
        OpenOptions::new().create(true).truncate(false).write(true).open(self.root.join(Self::LOCK))
    }
}

impl Deref for Staging {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        // Whatever was not moved into the store is garbage; failing to remove it leaves it to
        // collection.
        let _ = self.dir.remove();
    }
}
