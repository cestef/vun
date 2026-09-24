//! What the analysis knows at a point: the state of each local, of the parts of locals written
//! on their own, and of each borrow parameter's target.

use std::collections::{BTreeMap, BTreeSet};

use vun_grammar::PlaceState;
use vun_index::IndexVec;
use vun_intern::Symbol;
use vun_mir::Local;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Key {
    Field(Symbol),
    Position(usize),
    Payload(usize),
    Item(i128),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct States {
    pub(super) locals: IndexVec<Local, State>,
    /// Parts written on their own, by local and path.
    pub(super) parts: BTreeMap<(Local, Vec<Key>), State>,
    pub(super) targets: BTreeMap<Local, State>,
}

impl States {
    /// Joins the states of another path here; whether anything changed.
    pub(super) fn join(&mut self, other: &Self) -> bool {
        let before = self.clone();
        let keys: BTreeSet<_> = self.parts.keys().chain(other.parts.keys()).cloned().collect();
        for key in keys {
            let mine = self.parts.get(&key).copied().unwrap_or(self.locals[key.0]);
            let theirs = other.parts.get(&key).copied().unwrap_or(other.locals[key.0]);
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

    /// The state of the part `path` of `local`: `init` when the whole is, or when the part or
    /// one enclosing it was written; the whole's state otherwise.
    pub(super) fn of(&self, local: Local, path: Option<&[Key]>) -> State {
        let whole = self.locals[local];
        if whole == State::Init {
            return whole;
        }
        let written = path.is_some_and(|path| {
            (1..=path.len()).any(|n| self.parts.get(&(local, path[..n].to_vec())) == Some(&State::Init))
        });
        if written { State::Init } else { whole }
    }

    /// The binding of `local` goes out of scope: it holds nothing any more.
    pub(super) fn end(&mut self, local: Local) {
        self.locals[local] = State::Uninit;
        self.parts.retain(|(l, _), _| *l != local);
    }

    /// The whole of `local` is written.
    pub(super) fn write(&mut self, local: Local) {
        self.locals[local] = State::Init;
        self.parts.retain(|(l, _), _| *l != local);
    }
}
