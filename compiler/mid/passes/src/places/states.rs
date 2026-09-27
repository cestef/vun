//! What the analysis knows at a point: the state of each local, of the parts of locals written
//! on their own, and of each borrow parameter's target.

use std::collections::{BTreeMap, BTreeSet};

use vun_codec::{Decode, Encode};
use vun_desugar::PlaceState;
use vun_index::IndexVec;
use vun_intern::Symbol;
use vun_mir::Local;

use crate::flow::Fact;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub(super) enum State {
    Uninit,
    Init,
    /// `init` on some path here and `uninit` on another.
    Ambiguous,
}

impl State {
    fn join(self, other: Self) -> Self {
        if self == other { self } else { Self::Ambiguous }
    }

    /// The one of the two a read cannot rely on more: `uninit`, then undecided, then `init`.
    fn worse(self, other: Self) -> Self {
        let rank = |s| match s {
            Self::Init => 0,
            Self::Ambiguous => 1,
            Self::Uninit => 2,
        };
        if rank(other) > rank(self) { other } else { self }
    }
}

impl From<PlaceState> for State {
    fn from(state: PlaceState) -> Self {
        match state {
            PlaceState::Uninit => Self::Uninit,
            PlaceState::Init => Self::Init,
        }
    }
}

/// One step into a part of a local, known without running the body: a field, a tuple position,
/// a payload item, or an item at a constant index.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub(super) enum Key {
    Field(Symbol),
    Position(usize),
    /// A payload's part, of the variant it is of: parts of two variants are two places.
    Payload {
        variant: Symbol,
        at: usize,
    },
    Item(i128),
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(super) struct States {
    pub(super) locals: IndexVec<Local, State>,
    /// Parts written on their own, by local and path.
    pub(super) parts: BTreeMap<(Local, Vec<Key>), State>,
    pub(super) targets: BTreeMap<Local, State>,
}

impl States {
    /// The state of the part `path` of `local`, the whole when not known: that of the part or of
    /// the closest part enclosing it that was written or moved on its own, else the whole's, and
    /// no better than that of any part within it.
    pub(super) fn of(&self, local: Local, path: Option<&[Key]>) -> State {
        let path = path.unwrap_or_default();
        let within = self
            .parts
            .iter()
            .filter(|((l, p), _)| *l == local && p.len() > path.len() && p.starts_with(path));
        within.fold(self.enclosing(local, path), |state, (_, &part)| state.worse(part))
    }

    /// The state of the closest part enclosing `path` of `local`, or of `path` itself, known on
    /// its own; the whole's otherwise.
    fn enclosing(&self, local: Local, path: &[Key]) -> State {
        let known = (1..=path.len()).rev().find_map(|n| self.parts.get(&(local, path[..n].to_vec())));
        known.copied().unwrap_or(self.locals[local])
    }

    /// The part `path` of `local`, the whole when empty, is now in `state`, parts within it too.
    pub(super) fn set(&mut self, local: Local, path: &[Key], state: State) {
        self.parts.retain(|(l, p), _| *l != local || !p.starts_with(path));
        if path.is_empty() {
            self.locals[local] = state;
        } else {
            self.parts.insert((local, path.to_vec()), state);
        }
    }
}

impl Fact for States {
    /// Joins the states of another path here; whether anything changed.
    fn join(&mut self, other: &Self) -> bool {
        let before = self.clone();
        let keys: BTreeSet<_> = self.parts.keys().chain(other.parts.keys()).cloned().collect();
        for key in keys {
            let (mine, theirs) = (self.enclosing(key.0, &key.1), other.enclosing(key.0, &key.1));
            self.parts.insert(key, mine.join(theirs));
        }
        for (local, state) in other.locals.iter() {
            self.locals[local] = self.locals[local].join(*state);
        }
        for (local, state) in &other.targets {
            if let Some(mine) = self.targets.get_mut(local) {
                *mine = mine.join(*state);
            }
        }
        *self != before
    }
}
