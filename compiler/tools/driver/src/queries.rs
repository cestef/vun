//! Queries saved between runs (spec 13, section 2): a session that persists them loads the memos
//! saved beside its root file when it loads that file, and saves its own there when it ends. The
//! file is named by the toolchain that wrote it, since the encoding is its own; memos are verified
//! before use, so a stale one is recomputed and never trusted.

use std::hash::Hash as _;
use std::path::{Path, PathBuf};

use vun_fs::Edit;
use vun_hash::Hasher;
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

    /// Loads the queries saved beside `root`, the first time a root file is loaded.
    pub(crate) fn restore(&mut self, root: &Path) {
        if self.persistence != Persistence::On {
            return;
        }
        let Some(path) = Self::saved_at(root) else { return };
        if let Ok(bytes) = std::fs::read(&path) {
            match self.db.load(bytes) {
                Ok(restored) => tracing::info!(restored, path = %path.display(), "saved queries loaded"),
                Err(unloaded) => tracing::info!(%unloaded, path = %path.display(), "saved queries skipped"),
            }
        }
        self.persistence = Persistence::At(path);
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
    }

    /// The file queries are saved in for `root`, named by the running toolchain: its path, size
    /// and time of change, which every build changes.
    fn saved_at(root: &Path) -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let meta = exe.metadata().ok()?;
        let mut hasher = Hasher::default();
        (exe.as_os_str().as_encoded_bytes(), meta.len(), meta.modified().ok()?).hash(&mut hasher);
        let dir = root.parent().unwrap_or(Path::new("")).join(vun_fs::WORK).join(QUERIES);
        Some(dir.join(hasher.finish_hash().digest()))
    }
}
