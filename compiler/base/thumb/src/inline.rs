//! Blocks of inline assembly (spec 07, section 6) read as Thumb's instructions: a block names
//! instructions, forms, widths, register classes and choices by their index in the table, whose
//! declaration order `generated` keeps. Bindings are general registers; no form takes memory, an
//! immediate, a label or a symbol.

use vun_isa::{AsmOperand, Blocks, Fit, Fixed, Given, Label, Misfit, Place};
use vun_inline::{FormId, Inst, OpId, RegRef, SizeId};

use crate::{Barrier, Class, Form, Kind, Mask, Op, Operand, Reg, Size, Special, Thumb};

impl Op {
    /// The instruction a block names, and the form it was checked against.
    #[must_use]
    pub fn entry(op: OpId, form: FormId) -> (Self, &'static Form) {
        let op = Self::ALL[usize::from(op.0)];
        (op, &op.forms()[usize::from(form.0)])
    }
}

impl Size {
    #[must_use]
    pub const fn of(size: SizeId) -> Self {
        Self::ALL[size.0 as usize]
    }

    /// Its index in the table, as a block names it.
    #[must_use]
    pub const fn id(self) -> SizeId {
        SizeId(self as u8)
    }
}

impl Blocks for Thumb {
    const ARCH: vun_items::Item = vun_items::lang::arch::thumb;
    type Reg = Reg;
    type Kind = Kind;
    type Operand<S: Copy> = Operand;

    fn kinds<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Kind> {
        Op::entry(inst.op, inst.form).1.kinds()
    }

    /// The special registers a form reads or writes are not general ones.
    fn implicit<V, P, S>(_: &Inst<V, P, S>) -> impl Iterator<Item = Reg> {
        std::iter::empty()
    }

    fn written<V, P, S>(_: &Inst<V, P, S>) -> Option<(usize, u32)> {
        None
    }

    fn general(r: RegRef) -> Option<Reg> {
        match Class::ALL[usize::from(r.class.0)] {
            Class::General => Some(Reg::ALL[usize::from(r.number)]),
            Class::Special => None,
        }
    }

    fn fixed(kind: Option<Kind>, named: Option<Reg>) -> Result<Option<Reg>, AsmOperand> {
        match kind {
            Some(Kind::R) | None => Ok(named),
            Some(_) => Err(AsmOperand::Unallocated),
        }
    }

    fn addressable<V, P, S>(_: &vun_inline::Mem<V, P, S>, _: Kind) -> Result<(), AsmOperand> {
        Err(AsmOperand::Unencodable)
    }

    fn fit<V: Clone, P: Clone, S: Copy>(
        op: OpId,
        form: FormId,
        fixed: Fixed,
        givens: &[Given<V, P, S>],
    ) -> Result<Fit<V, P, S>, Misfit> {
        crate::fit::fit(op, form, fixed, givens)
    }

    fn label<S: Copy>(_: Label) -> Operand {
        unreachable!("no Thumb form takes a label")
    }

    fn target<S: Copy>(_: S) -> Operand {
        unreachable!("no Thumb form takes a symbol")
    }

    fn operand<V, P, S: Copy>(
        operand: &vun_inline::Operand<V, P, S>,
        kind: Kind,
        place: &mut impl Place<Self, S>,
    ) -> Operand {
        match (operand, kind) {
            (vun_inline::Operand::Reg(r), _) => {
                Self::general(*r).map_or(Operand::Special(Special::ALL[usize::from(r.number)]), Operand::Reg)
            }
            (vun_inline::Operand::Bound(_), _) => Operand::Reg(place.bound()),
            (vun_inline::Operand::Choice(k), Kind::Mask) => Operand::Mask(Mask::ALL[usize::from(*k)]),
            (vun_inline::Operand::Choice(k), _) => Operand::Barrier(Barrier::ALL[usize::from(*k)]),
            _ => panic!("no Thumb form takes an immediate, a condition, memory, a label or a symbol"),
        }
    }
}
