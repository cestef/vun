//! Queries saved between runs (spec 13, section 2): a session that persists them loads the memos
//! saved beside its root file when it loads that file, and saves its own there when it ends. The
//! file is named by the toolchain that wrote it, since the encoding is its own; memos are verified
//! before use, so a stale one is recomputed and never trusted.
//!
//! The memos of the libraries a project builds on, `lang`, `std` and its dependencies, are shared
//! between projects: those reading nothing of the project are saved in an entry of the shared
//! directory for the toolchain, the target chosen and the checks, which a session holding no
//! memos of its own loads, and which a session that computed some the entry lacked writes anew
//! when it ends.

use std::hash::Hash as _;
use std::path::{Path, PathBuf};

use vun_fs::{Edit, Tree};
use vun_hash::{Hasher, Stable};
use vun_query::Db;
use vun_trace::tracing;

use crate::Session;

/// Where saved queries are kept, in the toolchain's directory beside the root file (spec 12,
/// section 1).
const QUERIES: &str = "queries";

/// Every query whose memos persist, by the crate declaring them.
const PERSISTED: &[&[fn(&Db)]] = &[
    vun_hir::db::PERSISTED,
    vun_sema::PERSISTED,
    vun_sanitize::PERSISTED,
    vun_mir::PERSISTED,
    vun_mir_passes::PERSISTED,
    vun_lower::PERSISTED,
];

/// Whether a session's queries persist, and where once its root file is loaded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Persistence {
    #[default]
    Off,
    /// On, the file not chosen yet.
    On,
    /// Loaded from, and saved to, this file.
    At(PathBuf),
}

impl Session {
    /// Registers every persisted query with `db`.
    pub(crate) fn register(db: &Db) {
        for register in PERSISTED.iter().copied().flatten() {
            register(db);
        }
    }

    /// Loads the queries saved beside `root` the first time a root file is loaded, or, when none
    /// are, the libraries' memos shared between projects.
    pub(crate) fn restore(&mut self, root: &Path) {
        let own = match self.persistence {
            Persistence::On => Self::saved_at(root),
            Persistence::Off | Persistence::At(_) => None,
        };
        if let Some(path) = &own {
            self.persistence = Persistence::At(path.clone());
        }
        if self.db.restored() {
            return;
        }
        let loaded = own.is_some_and(|path| self.load_from(&path));
        if !loaded && let Some(path) = self.shared_entry() {
            self.load_from(&path);
        }
    }

    /// Loads the memos saved in `path`, if any: whether they were loaded.
    fn load_from(&self, path: &Path) -> bool {
        let Ok(bytes) = std::fs::read(path) else { return false };
        match self.db.load(bytes) {
            Ok(restored) => {
                tracing::info!(restored, path = %path.display(), "saved queries loaded");
                true
            }
            Err(unloaded) => {
                tracing::info!(%unloaded, path = %path.display(), "saved queries skipped");
                false
            }
        }
    }

    /// Writes the libraries' memos this session computed into the entry shared between
    /// projects, with those it loaded, when it computed some the entry lacked.
    pub fn share(&self) {
        let Some(path) = self.shared_entry() else { return };
        let Some(bytes) = self.db.save_shared() else { return };
        match path.put(&bytes) {
            Ok(()) => tracing::info!(bytes = bytes.len(), path = %path.display(), "shared queries saved"),
            Err(error) => tracing::warn!(%error, path = %path.display(), "cannot save the shared queries"),
        }
        // What other toolchains shared is never read by this one.
        let (Some(toolchain), Some(shared)) = (path.parent(), self.shared.as_deref()) else { return };
        for stale in shared.entries().unwrap_or_default().iter().filter(|p| *p != toolchain) {
            if let Err(error) = stale.remove() {
                tracing::debug!(%error, path = %stale.display(), "cannot remove shared queries");
            }
        }
    }

    /// The entry of the shared directory for the running toolchain, in a directory of its own,
    /// the target chosen and the checks, if the session shares memos.
    fn shared_entry(&self) -> Option<PathBuf> {
        let dir = self.shared.as_ref()?.join(Self::toolchain()?.finish_hash().digest());
        let mut hasher = Hasher::default();
        (Stable::of(&self.choice), Stable::of(&self.checks())).hash(&mut hasher);
        Some(dir.join(hasher.finish_hash().digest()))
    }

    /// Saves the session's queries where they were loaded from, when they changed.
    pub fn save(&self) {
        let Persistence::At(path) = &self.persistence else { return };
        if !self.db.changed() {
            tracing::debug!(path = %path.display(), "queries unchanged");
            return;
        }
        let bytes = self.db.save();
        match path.put(&bytes) {
            Ok(()) => tracing::info!(bytes = bytes.len(), path = %path.display(), "queries saved"),
            Err(error) => tracing::warn!(%error, path = %path.display(), "cannot save the queries"),
        }
        // What other toolchains saved is never read by this one; the next build of that one
        // computes its queries again.
        let saved = path.parent().and_then(|dir| dir.entries().ok()).unwrap_or_default();
        for stale in saved.iter().filter(|p| *p != path) {
            if let Err(error) = std::fs::remove_file(stale) {
                tracing::debug!(%error, path = %stale.display(), "cannot remove saved queries");
            }
        }
    }

    /// The file queries are saved in for `root`, named by the running toolchain: its path, size
    /// and time of change, which every build changes.
    #[must_use]
    pub fn saved_at(root: &Path) -> Option<PathBuf> {
        let dir = root.parent().unwrap_or(Path::new("")).join(vun_fs::WORK).join(QUERIES);
        Some(dir.join(Self::toolchain()?.finish_hash().digest()))
    }

    /// A hasher fed the running toolchain: its path, size and time of change, which every build
    /// changes.
    fn toolchain() -> Option<Hasher> {
        let exe = std::env::current_exe().ok()?;
        let meta = exe.metadata().ok()?;
        let mut hasher = Hasher::default();
        (exe.as_os_str().as_encoded_bytes(), meta.len(), meta.modified().ok()?).hash(&mut hasher);
        Some(hasher)
    }
}

/// A session ends sharing the libraries' memos it computed, whatever ran it.
impl Drop for Session {
    fn drop(&mut self) {
        self.share();
    }
}
