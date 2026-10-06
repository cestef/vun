//! Fitting a line's operands to a wasm form: a position takes what its kind says, a local bound
//! as its access reads or writes it, an immediate the bits hold; the width is the table's one.

use vun_inline::{Bound, ClassId, FormId, OpId};
use vun_isa::{Fit, Fixed, Given, Misfit, Scope, Slot, Stack, Why};

use crate::{Access, Arg, Class, Control, Kind, Op, Size, Ty};

impl Arg {
    /// Whether this position takes `given`.
    fn takes<V, P, S>(self, given: &Given<V, P, S>) -> Result<(), Why> {
        match (self.kind, given) {
            (Kind::Local, Given::Bound { bound, .. }) => match (self.access, bound) {
                (Access::Read, Bound::In { .. } | Bound::InOut { .. })
                | (Access::Write, Bound::Out { .. } | Bound::InOut { .. })
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
    let depth = givens.iter().find_map(|given| match given {
        Given::Imm(Some(depth)) => u32::try_from(*depth).ok(),
        _ => None,
    });
    let branch = |always| depth.map(|depth| Scope::Branch { depth, always });
    let scope = match form.control {
        Control::None => Some(Scope::None),
        Control::Open => Some(Scope::Open { two_armed: false }),
        Control::OpenIf => Some(Scope::Open { two_armed: true }),
        Control::Reopen => Some(Scope::Reopen),
        Control::Close => Some(Scope::Close),
        Control::Branch => branch(true),
        Control::BranchIf => branch(false),
    };
    let scope = scope.ok_or(Misfit::Operand { at: 0, why: Why::Wide })?;
    let stack = Stack { pops: slots(form.pops), pushes: slots(form.pushes), scope };
    let operands = givens.iter().cloned().map(Given::operand).collect();
    Ok(Fit::new(Size::B32.id(), operands).on(stack))
}
