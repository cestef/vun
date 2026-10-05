//! Fitting a line's operands to a wasm form: a position takes what its kind says, a local bound
//! as its access reads or writes it, an immediate the bits hold; the width is the table's one.

use vun_inline::{Bound, ClassId, FormId, OpId};
use vun_isa::{Fit, Fixed, Given, Misfit, Slot, Stack, Why};

use crate::{Access, Arg, Class, Kind, Op, Size, Ty};

impl Arg {
    /// Whether this position takes `given`.
    fn takes<V, P, S>(self, given: &Given<V, P, S>) -> Result<(), Why> {
        match (self.kind, given) {
            (Kind::Local, Given::Bound { bound, .. }) => match (self.access, bound) {
                (Access::Read, Bound::In { .. })
                | (Access::Write, Bound::Out { .. })
                | (Access::ReadWrite, Bound::InOut { .. }) => Ok(()),
                _ => Err(Why::Kind),
            },
            (Kind::Imm(imm), Given::Imm(Some(value))) if imm.holds(*value) => Ok(()),
            (Kind::Imm(_), Given::Imm(_)) => Err(Why::Wide),
            _ => Err(Why::Kind),
        }
    }
}

impl Ty {
    /// The slot of this type, `local` being the class of the form's local.
    fn slot(self, local: Option<ClassId>) -> Slot {
        let class = |class: Class| if class == Class::Unknown { Slot::Any } else { Slot::Class(class.id()) };
        match self {
            Self::I32 => class(Class::I32),
            Self::I64 => class(Class::I64),
            Self::F32 => class(Class::F32),
            Self::F64 => class(Class::F64),
            Self::Local => local.map_or(Slot::Any, |id| class(Class::of(id))),
            Self::Any => Slot::Any,
        }
    }
}

/// The form `form` of `op` fitted to `givens`.
pub(crate) fn fit<V: Clone, P: Clone, S: Copy>(
    op: OpId,
    form: FormId,
    _: Fixed,
    givens: &[Given<V, P, S>],
) -> Result<Fit<V, P, S>, Misfit> {
    let form = Op::entry(op, form).1;
    if form.args.len() != givens.len() {
        return Err(Misfit::Count);
    }
    let mut local = None;
    for (at, (arg, given)) in form.args.iter().zip(givens).enumerate() {
        arg.takes(given).map_err(|why| Misfit::Operand { at, why })?;
        if arg.kind == Kind::Local {
            // A local is held as the type of the value bound to it.
            local = given.class();
        }
    }
    let slots = |tys: &[Ty]| tys.iter().map(|ty| ty.slot(local)).collect();
    let stack = Stack { pops: slots(form.pops), pushes: slots(form.pushes) };
    let operands = givens.iter().cloned().map(Given::operand).collect();
    Ok(Fit::new(Size::B32.id(), operands).on(stack))
}
