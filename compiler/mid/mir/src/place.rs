//! Places: what statements write, operands read and borrows take. A place is a local and the
//! projections applied to it in order: through a borrow or pointer, into a field, or to an item.

use overt_intern::Symbol;

use crate::body::Local;

/// A storage location.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: Local,
    pub projection: Vec<Projection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Projection {
    /// `.*`: what a borrow or pointer points to.
    Deref,
    Field(Symbol),
    /// The item at the index held in a local.
    Index(Local),
    /// The item at a fixed position of a tuple.
    Position(usize),
    /// An item of the payload of a variant, once tested to be one.
    Payload(usize),
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

    /// Whether this place and `other` may share storage: one contains the other. Distinct
    /// fields, positions and payload items are disjoint; items at run-time indices are not
    /// told apart.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.local == other.local
            && self.projection.iter().zip(&other.projection).all(|pair| match pair {
                (Projection::Field(a), Projection::Field(b)) => a == b,
                (Projection::Position(a), Projection::Position(b))
                | (Projection::Payload(a), Projection::Payload(b)) => a == b,
                _ => true,
            })
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
