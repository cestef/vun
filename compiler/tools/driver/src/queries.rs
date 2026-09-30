//! Queries saved between runs (spec 13, section 2): a session that persists them loads the memos
//! saved beside its root file when it loads that file, and saves its own there when it ends. The
//! file is named by the toolchain that wrote it, since the encoding is its own; memos are verified
//! before use, so a stale one is recomputed and never trusted.
//!
//! The memos of the libraries a project builds on, `lang`, `std` and its dependencies, are shared
//! between projects: those reading nothing of the project are saved in a file of the shared
//! directory for the toolchain, the target chosen and the checks, named by its contents, which a
//! session that computed some the newest file lacked writes when it ends. A project's own memos
//! are saved layered over the shared file loaded, holding what it lacks, and are loaded with it;
//! a session holding none of its own loads the newest. Older shared files are kept a while, so
//! that a project's file outlives the shared file it is layered over being outgrown.

use std::hash::Hash as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use vun_fs::{Edit, Tree};
use vun_hash::{Algorithm, Hash, Hasher, Stable};
use vun_query::{Db, Layering, Unloaded};
use vun_trace::tracing;

use crate::Session;

/// Where saved queries are kept, in the toolchain's directory beside the root file (spec 12,
/// section 1).
const QUERIES: &str = "queries";

/// How long a shared file other than the newest is kept after it was written: a project's file
/// layered over one removed is dropped, and its memos computed again.
const SHARED_KEPT: Duration = Duration::from_hours(7 * 24);

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

    /// Loads the queries saved beside `root` the first time a root file is loaded, with the
    /// shared file they are layered over, or, when there are none, the newest shared file.
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
        if let Some(path) = own
            && let Ok(bytes) = std::fs::read(&path)
        {
            let loaded = match Layering::of(&bytes).map(|layering| layering.base) {
                Ok(Some(base)) => {
                    match self.shared_dir().and_then(|dir| std::fs::read(dir.join(base.digest())).ok()) {
                        Some(shared) => self.db.load_over(shared, Some(bytes)),
                        None => Err(Unloaded::Base),
                    }
                }
                Ok(None) => self.db.load(bytes),
                Err(unloaded) => Err(unloaded),
            };
            if Self::loaded(loaded, &path) {
                return;
            }
        }
        if let Some((_, path)) = self.shared_files().into_iter().next()
            && let Ok(bytes) = std::fs::read(&path)
        {
            Self::loaded(self.db.load_over(bytes, None), &path);
        }
    }

    /// Whether the memos of `path` were `loaded`, as logged.
    fn loaded(loaded: Result<usize, Unloaded>, path: &Path) -> bool {
        match loaded {
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

    /// Writes the libraries' memos this session computed into a file shared between projects,
    /// with those it loaded, when it computed some the file loaded lacked. Older files are
    /// removed once kept long enough.
    pub fn share(&self) {
        let Some(dir) = self.shared_dir() else { return };
        let Some(bytes) = self.db.save_shared() else { return };
        let Some(id) = Layering::of(&bytes).ok().and_then(|layering| layering.id) else { return };
        let path = dir.join(id.digest());
        match path.put(&bytes) {
            Ok(()) => tracing::info!(bytes = bytes.len(), path = %path.display(), "shared queries saved"),
            Err(error) => tracing::warn!(%error, path = %path.display(), "cannot save the shared queries"),
        }
        let now = SystemTime::now();
        let old = |written: &SystemTime| now.duration_since(*written).is_ok_and(|age| age > SHARED_KEPT);
        let files = self.shared_files();
        for (_, stale) in files.iter().skip(1).filter(|(written, _)| old(written)) {
            if let Err(error) = std::fs::remove_file(stale) {
                tracing::debug!(%error, path = %stale.display(), "cannot remove shared queries");
            }
        }
        // What other toolchains shared is never read by this one.
        let (Some(toolchain), Some(shared)) = (dir.parent(), self.shared.as_deref()) else { return };
        for stale in shared.entries().unwrap_or_default().iter().filter(|p| *p != toolchain) {
            if let Err(error) = stale.remove() {
                tracing::debug!(%error, path = %stale.display(), "cannot remove shared queries");
            }
        }
    }

    /// The directory of the shared files for the running toolchain, in a directory of its own,
    /// the target chosen and the checks, if the session shares memos.
    fn shared_dir(&self) -> Option<PathBuf> {
        let dir = self.shared.as_ref()?.join(Self::toolchain()?.finish_hash().digest());
        let mut hasher = Hasher::default();
        (Stable::of(&self.choice), Stable::of(&self.checks())).hash(&mut hasher);
        Some(dir.join(hasher.finish_hash().digest()))
    }

    /// The shared files for the running toolchain, target and checks, the newest first, with
    /// when each was written: those named by a hash, as written whole.
    fn shared_files(&self) -> Vec<(SystemTime, PathBuf)> {
        let Some(dir) = self.shared_dir() else { return Vec::new() };
        let named = |path: &PathBuf| {
            let name = path.file_name()?.to_str()?;
            Algorithm::ALL.iter().find_map(|&algorithm| Hash::from_digest(algorithm, name))?;
            std::fs::metadata(path).and_then(|meta| meta.modified()).ok()
        };
        let mut files: Vec<_> = dir
            .entries()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|path| Some((named(&path)?, path)))
            .collect();
        files.sort_by_key(|(written, _)| std::cmp::Reverse(*written));
        files
    }

    /// Saves the session's queries where they were loaded from, when they changed, after the
    /// libraries' memos it shares, so that its own are saved layered over those.
    pub fn save(&self) {
        self.share();
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
