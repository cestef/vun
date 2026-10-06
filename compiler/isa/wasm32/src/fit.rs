//! Fitting a line's operands to a wasm form: a position takes what its kind says, a local bound
//! as its access reads or writes it, an immediate the bits hold; the width is the table's one.

use vun_inline::{Bound, ClassId, FormId, OpId, Operand};
use vun_isa::{Fit, Fixed, Given, Misfit, Nest, Scope, Slot, Stack, Why};

use crate::{Access, Arg, Choice, Class, Control, Global, Imm, Kind, Op, Size, Ty, Yields};

/// The depth of a block, an unsigned 32-bit immediate.
const DEPTH: Imm = Imm::Unsigned(32);

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
            (Kind::Depths, Given::Imm(Some(value))) if DEPTH.holds(*value) => Ok(()),
            (Kind::Imm(Imm::Bits(bits)), Given::Float(float)) if float.at(bits).is_some() => Ok(()),
            (Kind::Imm(_) | Kind::Depths, Given::Imm(_)) => Err(Why::Wide),
            (Kind::Symbol, Given::Symbol(_)) => Ok(()),
            (Kind::Choice(choice), Given::Choice { of, index })
                if of.0 == choice as u8 && usize::from(*index) < choice.count() =>
            {
                Ok(())
            }
            _ => Err(Why::Kind),
        }
    }
}

impl Choice {
    /// How many choices of this kind there are.
    const fn count(self) -> usize {
        match self {
            Self::Yields => Yields::ALL.len(),
            Self::Global => Global::ALL.len(),
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
    let variadic = form.args.last().is_some_and(|arg| arg.kind == Kind::Depths);
    if givens.len() < form.args.len() || (givens.len() > form.args.len() && !variadic) {
        return Err(Misfit::Count);
    }
    let mut local = None;
    for (at, given) in givens.iter().enumerate() {
        let arg = form.args.get(at).or(form.args.last()).expect("a position of each given");
        arg.takes(given).map_err(|why| Misfit::Operand { at, why })?;
        if arg.kind == Kind::Local {
            // A local is held as the type of the value bound to it.
            local = given.class();
        }
    }
    let slots = |tys: &[Ty]| tys.iter().map(|ty| ty.slot(local)).collect();
    let depths: Vec<u32> = givens
        .iter()
        .filter_map(|given| match given {
            Given::Imm(Some(depth)) => u32::try_from(*depth).ok(),
            _ => None,
        })
        .collect();
    let yields = givens.iter().find_map(|given| match given {
        Given::Choice { index, .. } => Some(Slot::Class(Yields::ALL[usize::from(*index)].class().id())),
        _ => None,
    });
    let branch = |always| depths.first().map(|&depth| Scope::Branch { depth, always });
    let open = |nest| Some(Scope::Open { nest, yields });
    let scope = match form.control {
        Control::None => Some(Scope::None),
        Control::Open => open(Nest::Plain),
        Control::OpenLoop => open(Nest::Loop),
        Control::OpenIf => open(Nest::If),
        Control::Reopen => Some(Scope::Reopen),
        Control::Close => Some(Scope::Close),
        Control::Branch => branch(true),
        Control::BranchIf => branch(false),
        Control::BranchTable => Some(Scope::Table(depths)),
        Control::Call => Some(Scope::Call),
    };
    let scope = scope.ok_or(Misfit::Operand { at: 0, why: Why::Wide })?;
    let stack = Stack { pops: slots(form.pops), pushes: slots(form.pushes), scope };
    let operands = givens.iter().enumerate().map(|(at, given)| {
        let arg = form.args.get(at).or(form.args.last());
        match (given, arg.map(|arg| arg.kind)) {
            (Given::Float(float), Some(Kind::Imm(Imm::Bits(bits)))) => {
                Operand::Imm(i64::from_ne_bytes(float.at(bits).unwrap_or_default().to_ne_bytes()))
            }
            _ => given.clone().operand(),
        }
    });
    Ok(Fit::new(Size::B32.id(), operands.collect()).on(stack))
}
