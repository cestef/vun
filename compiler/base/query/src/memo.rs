//! A memoized result: the value, its fingerprint, what computing it read, and the revision it was
//! last known current in, which any thread verifying it records.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use vun_codec::{Blob, Context, Malformed};
use vun_trace::{Activity, tracing};

use super::db::{Db, Sharing};
use super::fingerprint::Fingerprint;
use super::groups::{Groups, Reach};
use super::persist::{SavedDep, Under};
use super::table::{Slot, TableId};
use super::tally::Event;
use super::{Query, Revision};

pub(crate) struct Memo<Q: Query> {
    /// Unset for a memo loaded without its value, or whose saved value is not read yet.
    value: OnceLock<Q::Value>,
    /// The saved value of a loaded memo, read when first needed.
    pub(crate) saved: Option<Saved<Q>>,
    pub(crate) fingerprint: Fingerprint,
    /// Each memo read, once, in the order of the first reads.
    pub(crate) deps: Deps,
    /// None for a loaded memo until it is verified.
    pub(crate) verified: Verified,
    pub(crate) source: Source,
    /// How far the inputs it reads, through all it read, are shared.
    pub(crate) durability: Durability,
    pub(crate) novelty: Novel,
}

/// Whether a memo is new to the saved memos loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Novelty {
    /// Loaded, or computed again equal to one loaded.
    Saved,
    /// Computed, with none loaded equal to it.
    New,
}

/// A memo's [`Novelty`], which becomes [`Novelty::Saved`] once memos written from it are loaded.
pub(crate) struct Novel(AtomicBool);

impl Novel {
    pub(crate) fn new(novelty: Novelty) -> Self {
        Self(AtomicBool::new(novelty == Novelty::New))
    }

    pub(crate) fn get(&self) -> Novelty {
        if self.0.load(Ordering::Relaxed) { Novelty::New } else { Novelty::Saved }
    }

    pub(crate) fn saved(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Where a memo's value comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Computed,
    /// An input set ([`Db::set_as`]), shared as it says.
    Set(Sharing),
    /// An input's value while it is not set, alike in every database.
    Unset,
}

/// Which inputs a memo reads, through all it read: the groups of the shared ones, or none when
/// one is the database's own. A memo whose inputs are shared holds as long as the inputs of those
/// groups do, which their fingerprint tells ([`Db::inputs_of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Durability {
    pub(crate) groups: Option<Reach>,
    /// Loaded: the fingerprint of the inputs of those groups when it last held.
    pub(crate) under: Option<Fingerprint>,
}

impl Durability {
    pub(crate) const OWN: Self = Self { groups: None, under: None };

    pub(crate) fn reading(groups: Reach) -> Self {
        Self { groups: Some(groups), under: None }
    }

    /// What a saved memo holds of it.
    pub(crate) fn saved(under: Option<Under>) -> Self {
        under.map_or(Self::OWN, |Under { groups, inputs }| Self { groups: Some(groups), under: Some(inputs) })
    }
}

/// A value as saved, a place in the stream the database loaded, and how to read it.
pub(crate) struct Saved<Q: Query> {
    pub(crate) at: Blob,
    pub(crate) read: ReadValue<Q::Value>,
}

/// Reads a value from its place in a stream.
pub(crate) type ReadValue<V> = fn(&Context, Blob) -> Result<V, Malformed>;

/// A memo read while another was computed, with the fingerprint it had then.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Dep {
    pub(crate) table: TableId,
    pub(crate) slot: Slot,
    pub(crate) fingerprint: Fingerprint,
}

impl<Q: Query> Memo<Q> {
    /// An input's value, set or standing while it is not.
    pub(crate) fn input(
        value: Q::Value,
        fingerprint: Fingerprint,
        revision: Revision,
        source: Source,
        group: Fingerprint,
    ) -> Self {
        let verified = Verified::at(revision);
        let durability = match source {
            Source::Set(Sharing::Own) => Durability::OWN,
            Source::Set(_) | Source::Computed | Source::Unset => Durability::reading(Groups::only(group)),
        };
        Self {
            value: OnceLock::from(value),
            saved: None,
            fingerprint,
            deps: Deps::Read(Arc::from([])),
            verified,
            source,
            durability,
            novelty: Novel::new(Novelty::New),
        }
    }

    pub(crate) fn computed(
        value: Q::Value,
        fingerprint: Fingerprint,
        deps: Arc<[Dep]>,
        revision: Revision,
        durability: Durability,
        novelty: Novelty,
    ) -> Self {
        let verified = Verified::at(revision);
        let deps = Deps::Read(deps);
        Self {
            value: OnceLock::from(value),
            saved: None,
            fingerprint,
            deps,
            verified,
            source: Source::Computed,
            durability,
            novelty: Novel::new(novelty),
        }
    }

    /// A memo of another process, never verified in this one; `under`, for one whose inputs are
    /// shared, their groups and their fingerprint when it last held.
    pub(crate) fn loaded(
        saved: Option<Saved<Q>>,
        fingerprint: Fingerprint,
        deps: Blob,
        under: Option<Under>,
    ) -> Self {
        let verified = Verified::default();
        let deps = Deps::Saved { at: deps, read: OnceLock::new() };
        let durability = Durability::saved(under);
        Self {
            value: OnceLock::new(),
            saved,
            fingerprint,
            deps,
            verified,
            source: Source::Computed,
            durability,
            novelty: Novel::new(Novelty::Saved),
        }
    }

    /// Whether it is an input's value rather than computed.
    pub(crate) fn is_input(&self) -> bool {
        self.source != Source::Computed
    }

    /// Whether it was computed in this process, and no memo loaded is equal to it.
    pub(crate) fn fresh(&self) -> bool {
        self.novelty.get() == Novelty::New && !self.is_input()
    }

    /// Whether it holds in `revision` without looking at what it read.
    pub(crate) fn current(&self, revision: Revision) -> bool {
        self.is_input() || self.verified.get() == Some(revision)
    }

    /// Its value if it has one: computed, or saved and read now the first time. `None` when it
    /// was loaded without it, or its saved form does not read.
    pub(crate) fn value(&self, db: &Db) -> Option<&Q::Value> {
        if let Some(value) = self.value.get() {
            return Some(value);
        }
        let saved = self.saved.as_ref()?;
        let _timer = Activity::read(Q::NAME).start();
        db.table::<Q>().counts.add(Event::ValueRead);
        match (saved.read)(&db.store.get()?.stream, saved.at) {
            Ok(value) => Some(self.value.get_or_init(|| value)),
            Err(malformed) => {
                tracing::debug!(query = Q::NAME, %malformed, "saved value not read");
                None
            }
        }
    }

    /// Its value if computed or read already, without reading it.
    pub(crate) fn known(&self) -> Option<&Q::Value> {
        self.value.get()
    }
}

/// What a memo read: as recorded, or as saved, positions in the file loaded that are brought to
/// slots of this process when first needed.
pub(crate) enum Deps {
    Read(Arc<[Dep]>),
    Saved { at: Blob, read: OnceLock<Option<Box<[Dep]>>> },
}

impl Deps {
    /// The memos read, by slot; `None` when one of them is not held by this process's queries.
    pub(crate) fn get(&self, db: &Db) -> Option<&[Dep]> {
        match self {
            Self::Read(deps) => Some(&**deps),
            Self::Saved { at, read } => read
                .get_or_init(|| {
                    let store = db.store.get()?;
                    let saved = store
                        .stream
                        .read_blob::<Vec<SavedDep>>(*at)
                        .inspect_err(|malformed| tracing::debug!(%malformed, "saved dependencies not read"))
                        .ok()?;
                    saved.into_iter().map(|dep| store.dep(db, dep)).collect()
                })
                .as_deref(),
        }
    }
}

/// A revision, or none: `0` stands for none and `n + 1` for revision `n`.
#[derive(Default)]
pub(crate) struct Verified(AtomicU64);

impl Verified {
    pub(crate) fn at(revision: Revision) -> Self {
        Self(AtomicU64::new(revision.0 + 1))
    }

    pub(crate) fn get(&self) -> Option<Revision> {
        self.0.load(Ordering::Acquire).checked_sub(1).map(Revision)
    }

    pub(crate) fn set(&self, revision: Revision) {
        self.0.store(revision.0 + 1, Ordering::Release);
    }
}
