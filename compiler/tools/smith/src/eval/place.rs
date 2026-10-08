//! Places written: the variable a local is, or what a reference it holds lends, and the part
//! of its value that the steps reach.

use super::{Frame, Machine, Trap};
use crate::ir::{FnId, Place, Projection};
use crate::value::Value;

impl Machine<'_> {
    /// `change` applied to the value at `place`, in place.
    pub(super) fn at_place<R>(
        &mut self,
        place: &Place,
        frame: &mut Frame,
        within: FnId,
        change: impl FnOnce(&mut Value) -> Result<R, Trap>,
    ) -> Result<R, Trap> {
        let mut steps = Vec::new();
        for step in &place.steps {
            steps.push(match step {
                Projection::Field(at) | Projection::Member(at) => *at,
                Projection::Index(index) => {
                    let value = self.eval(index, frame, within)?;
                    let n = value.int().ok_or(Trap::Shape)?;
                    usize::try_from(n).map_err(|_| Trap::Index(n.clone(), 0))?
                }
            });
        }
        let mut cell = frame.cell(place.local)?;
        if place.deref {
            let Value::Ref(held) = cell.0.borrow().clone() else { return Err(Trap::Shape) };
            cell = held;
        }
        let mut whole = cell.0.borrow_mut();
        let mut part = &mut *whole;
        for at in steps {
            let (Value::Tuple(parts) | Value::Array(parts)) = part else { return Err(Trap::Shape) };
            let count = parts.len();
            part = parts.get_mut(at).ok_or_else(|| Trap::Index(at.into(), count))?;
        }
        change(part)
    }
}
