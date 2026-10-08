//! Places an assignment writes and a borrow lends: a local, or what a reference held by a local
//! refers to, and then parts of it.

use super::{Expr, LocalId, Untyped};
use crate::int::{BigInt, Range};
use crate::ty::Ty;

/// A step from a place into a part of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Projection {
    /// A tuple's item, written `.0`.
    Field(usize),
    /// A struct's field, written by its name.
    Member(usize),
    /// An array's item, the index of which is in bounds.
    Index(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub local: LocalId,
    /// Whether the local holds a reference, and the place is what it refers to.
    pub deref: bool,
    pub steps: Vec<Projection>,
}

impl Place {
    /// The place of `steps` into `local`, of type `root`, and its type.
    ///
    /// # Errors
    /// Returns why the steps do not lead into the type.
    pub fn new(
        local: LocalId,
        root: &Ty,
        deref: bool,
        steps: Vec<Projection>,
    ) -> Result<(Self, Ty), Untyped> {
        let mut ty = if deref {
            let Ty::Ref(_, inner) = root else { return Err(Untyped::Operand) };
            (**inner).clone()
        } else {
            root.clone()
        };
        for step in &steps {
            ty = match (step, &ty) {
                (Projection::Field(at), Ty::Tuple(parts))
                | (Projection::Member(at), Ty::Struct(_, parts)) => {
                    parts.get(*at).ok_or(Untyped::Bounds)?.clone()
                }
                (Projection::Index(index), Ty::Array(len, item)) => {
                    let last = BigInt::from(*len) - BigInt::from(1);
                    let range = index.ty.range().ok_or(Untyped::Operand)?;
                    range.within(&Range::new(0, last)).then_some(()).ok_or(Untyped::Bounds)?;
                    (**item).clone()
                }
                _ => return Err(Untyped::Operand),
            };
        }
        Ok((Self { local, deref, steps }, ty))
    }

    /// The expressions of the steps.
    pub fn indices_mut(&mut self) -> impl Iterator<Item = &mut Expr> {
        self.steps.iter_mut().filter_map(|step| match step {
            Projection::Index(index) => Some(&mut **index),
            _ => None,
        })
    }
}
