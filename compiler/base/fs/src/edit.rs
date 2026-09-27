//! Changes to paths that create what they need and treat an absent path as already removed.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Changes on paths.
pub trait Edit {
    /// Writes `contents` as this file, creating its parent directories. The file is written beside
    /// and renamed over this one, so a reader sees the old contents or the new, never a part.
    ///
    /// # Errors
    /// Returns an error if a directory or the file cannot be written.
    fn put(&self, contents: impl AsRef<[u8]>) -> io::Result<()>;

    /// Removes this directory and everything in it; an absent path is already removed.
    ///
    /// # Errors
    /// Returns an error if something present cannot be removed.
    fn remove(&self) -> io::Result<()>;

    /// Makes this an empty directory, removing what was there.
    ///
    /// # Errors
    /// Returns an error if the old directory cannot be removed or the new one created.
    fn fresh(&self) -> io::Result<()>;

    /// Moves the directory `from` here, replacing what was there and creating the parents. What
    /// was there is moved aside before it is removed, so this is never missing for long.
    ///
    /// # Errors
    /// Returns an error if the old directory cannot be moved or removed, or `from` cannot be moved.
    fn replace_with(&self, from: &Path) -> io::Result<()>;

    /// Moves the directory `from` here unless something already is, then removes `from`: for
    /// entries named by their contents, where whatever is here is equal to `from` and may be in
    /// use by another process, so it is kept.
    ///
    /// # Errors
    /// Returns an error if `from` can be neither moved here nor removed.
    fn install(&self, from: &Path) -> io::Result<()>;

    /// Makes this a symbolic link to `target`, creating its parents; like [`Edit::put`], the link
    /// is made beside and renamed over what was here.
    ///
    /// # Errors
    /// Returns an error if a directory or the link cannot be written, or the system has no
    /// symbolic links.
    fn link_to(&self, target: &Path) -> io::Result<()>;
}

/// A file being written by [`Edit::put`] ends with this, after the writing process's id and a
/// count of the parts it named, so that processes and threads writing one file at once each write
/// their own.
const PART: &str = "part";

/// The parts this process named so far.
static PARTS: AtomicU64 = AtomicU64::new(0);

/// What [`Edit::put`] needs of a path beyond [`Edit`].
trait Part {
    /// The name of the file `put` writes before renaming it to this path.
    fn part_name(&self) -> OsString;
}

impl Part for Path {
    fn part_name(&self) -> OsString {
        let mut name = OsString::from(".");
        name.push(self.file_name().unwrap_or_default());
        name.push(format!(".{}.{}.{PART}", std::process::id(), PARTS.fetch_add(1, Ordering::Relaxed)));
        name
    }
}

impl Edit for Path {
    fn put(&self, contents: impl AsRef<[u8]>) -> io::Result<()> {
        if let Some(parent) = self.parent() {
            fs::create_dir_all(parent)?;
        }
        let part = self.with_file_name(Self::part_name(self));
        let write = || -> io::Result<()> {
            // A new file, so that a link planted at the part's name is never written through.
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&part)?;
            file.write_all(contents.as_ref())?;
            file.sync_all()?;
            fs::rename(&part, self)
        };
        write().inspect_err(|_| {
            // The error that matters is the write's or the rename's.
            let _ = fs::remove_file(&part);
        })
    }

    fn remove(&self) -> io::Result<()> {
        match fs::remove_dir_all(self) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            done => done,
        }
    }

    fn fresh(&self) -> io::Result<()> {
        self.remove()?;
        fs::create_dir_all(self)
    }

    fn replace_with(&self, from: &Path) -> io::Result<()> {
        if let Some(parent) = self.parent() {
            fs::create_dir_all(parent)?;
        }
        let aside = self.with_file_name(Self::part_name(self));
        let moved = match fs::rename(self, &aside) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => false,
            moved => moved.map(|()| true)?,
        };
        fs::rename(from, self).inspect_err(|_| {
            if moved {
                let _ = fs::rename(&aside, self);
            }
        })?;
        if moved { aside.remove() } else { Ok(()) }
    }

    fn install(&self, from: &Path) -> io::Result<()> {
        if let Some(parent) = self.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.symlink_metadata().is_err() {
            match fs::rename(from, self) {
                Ok(()) => return Ok(()),
                // Another process installed it first.
                Err(_) if self.symlink_metadata().is_ok() => {}
                Err(e) => return Err(e),
            }
        }
        from.remove()
    }

    fn link_to(&self, target: &Path) -> io::Result<()> {
        if let Some(parent) = self.parent() {
            fs::create_dir_all(parent)?;
        }
        let part = self.with_file_name(Self::part_name(self));
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(target, &part);
        #[cfg(not(unix))]
        let linked = Err(io::Error::from(io::ErrorKind::Unsupported));
        linked.and_then(|()| fs::rename(&part, self)).inspect_err(|_| {
            let _ = fs::remove_file(&part);
        })
    }
}
