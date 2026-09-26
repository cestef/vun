//! Places: what statements write, operands read and borrows take. A place is a local and the
//! projections applied to it in order: through a borrow or pointer, into a field, or to an item.

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;

use crate::body::Local;

/// A storage location.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Place {
    pub local: Local,
    pub projection: Vec<Projection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Projection {
    /// `.*`: what a borrow or pointer points to.
    Deref,
    Field(Symbol),
    /// The item at the index held in a local.
    Index(Local),
    /// The item at a fixed position of a tuple.
    Position(usize),
    /// The item at `at` of the payload of the variant `variant`, once tested to be one.
    Payload {
        variant: Symbol,
        at: usize,
    },
}

impl Place {
    /// The whole local `local`.
    #[must_use]
    pub fn of(local: Local) -> Self {
        Self { local, projection: Vec::new() }
    }

    /// This place, projected by `projection`.
    #[must_use]
    pub fn project(mut self, projection: Projection) -> Self {
        self.projection.push(projection);
        self
    }

    /// Adds the locals reading this place reads to `out`: its local, when it is read whole or
    /// projected, and the indices it uses.
    pub(crate) fn read_by(&self, whole: bool, out: &mut Vec<Local>) {
        if whole || !self.projection.is_empty() {
            out.push(self.local);
        }
        out.extend(self.indices());
    }

    /// Whether this place and `other` may share storage: one contains the other. Distinct
    /// fields, positions and payload items are disjoint; items at run-time indices are not
    /// told apart.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.local == other.local
            && self.projection.iter().zip(&other.projection).all(|pair| match pair {
                (Projection::Field(a), Projection::Field(b)) => a == b,
                (Projection::Position(a), Projection::Position(b)) => a == b,
                // The payloads of different variants share storage.
                (Projection::Payload { variant: va, at: a }, Projection::Payload { variant: vb, at: b }) => {
                    va != vb || a == b
                }
                _ => true,
            })
    }

    /// Whether this place lies within `outer` but past a `.*` below it: in storage `outer`
    /// points to, not in its own.
    #[must_use]
    pub fn beyond(&self, outer: &Self) -> bool {
        self.local == outer.local
            && self.projection.starts_with(&outer.projection)
            && self.projection[outer.projection.len()..].contains(&Projection::Deref)
    }

    /// Whether this place is reached through a borrow or pointer: storage outside its local.
    #[must_use]
    pub fn is_indirect(&self) -> bool {
        self.projection.contains(&Projection::Deref)
    }

    /// The locals holding the indices this place reads.
    pub fn indices(&self) -> impl Iterator<Item = Local> + '_ {
        self.projection.iter().filter_map(|p| match p {
            Projection::Index(local) => Some(*local),
            _ => None,
        })
    }
}
