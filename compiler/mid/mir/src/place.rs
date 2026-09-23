//! Places: what statements write and operands read.

use crate::body::Local;

/// A storage location. Only whole locals so far; projections (fields, indices, dereferences)
/// come with aggregates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: Local,
}
