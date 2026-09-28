//! Blocks of inline assembly (spec 07, section 6) read as Thumb's instructions: a block names
//! instructions, forms, widths, conditions and register classes by their index in the table,
//! whose declaration order `generated` keeps. Bindings are general registers; memory is in the
//! addressing mode its form names, and a symbol a branch's target or half of an address.

use vun_inline::{Disp, FormId, Inst, OpId, RegRef, SizeId};
use vun_isa::{AsmOperand, Blocks, Fit, Fixed, Given, Label, Misfit, Place};

use crate::{
    Access, Barrier, Cc, Class, Form, Kind, Mask, Mem, Mode, Op, Operand, Reg, Size, Special, Status, Thumb, V,
};

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
}

impl Blocks for Thumb {
    const ARCH: vun_items::Item = vun_items::lang::arch::thumb;
    type Reg = Reg;
    type Kind = Kind;
    type Operand<S: Copy> = Operand<S>;

    fn kinds<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Kind> {
        Op::entry(inst.op, inst.form).1.kinds()
    }

    fn implicit<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Reg> {
        let form = Op::entry(inst.op, inst.form).1;
        form.reads.iter().chain(form.writes).copied()
    }

    fn written<V, P, S>(inst: &Inst<V, P, S>) -> Option<(usize, u32)> {
        let form = Op::entry(inst.op, inst.form).1;
        form.args.iter().enumerate().find_map(|(i, arg)| match (arg.kind, arg.access) {
            (Kind::Mem(_, width), Access::Write | Access::ReadWrite) => {
                Some((i, width.at(Size::of(inst.size)).bytes()))
            }
            _ => None,
        })
    }

    fn general(r: RegRef) -> Option<Reg> {
        match Class::ALL[usize::from(r.class.0)] {
            Class::General => Some(Reg::ALL[usize::from(r.number)]),
            Class::Fp | Class::Special | Class::Status => None,
        }
    }

    fn fixed(kind: Option<Kind>, named: Option<Reg>) -> Result<Option<Reg>, AsmOperand> {
        match kind {
            Some(Kind::Gpr(_) | Kind::Split | Kind::Shifted) | None => Ok(named),
            Some(_) => Err(AsmOperand::Unallocated),
        }
    }

    /// Off a base register, or indexed in `index` and `narrow_index`.
    fn addressable<V, P, S>(mem: &vun_inline::Mem<V, P, S>, kind: Kind) -> Result<(), AsmOperand> {
        let Kind::Mem(mode, _) = kind else { return Err(AsmOperand::Unencodable) };
        let indexed = matches!(mode, Mode::Index | Mode::NarrowIndex);
        match (&mem.base, &mem.index, &mem.disp) {
            (None, _, _) => Err(AsmOperand::Absolute),
            (Some(_), Some((_, 1)), Disp::Bytes(0)) if indexed => Ok(()),
            (Some(_), None, Disp::Bytes(_)) if !indexed => Ok(()),
            (Some(_), Some(_), _) => Err(AsmOperand::Indexed),
            (Some(_), None, Disp::Symbol(..)) => Err(AsmOperand::SymbolOffRegister),
            _ => Err(AsmOperand::Unencodable),
        }
    }

    fn fit<V: Clone, P: Clone, S: Copy>(
        op: OpId,
        form: FormId,
        fixed: Fixed,
        givens: &[Given<V, P, S>],
    ) -> Result<Fit<V, P, S>, Misfit> {
        crate::fit::fit(op, form, fixed, givens)
    }

    fn label<S: Copy>(label: Label) -> Operand<S> {
        Operand::Label(label)
    }

    fn target<S: Copy>(target: S) -> Operand<S> {
        Operand::Sym(target)
    }

    fn operand<Vb, P, S: Copy>(
        operand: &vun_inline::Operand<Vb, P, S>,
        kind: Kind,
        place: &mut impl Place<Self, S>,
    ) -> Operand<S> {
        match operand {
            vun_inline::Operand::Reg(r) => {
                let n = usize::from(r.number);
                match Class::ALL[usize::from(r.class.0)] {
                    Class::General => Operand::Reg(Reg::ALL[n]),
                    Class::Fp => Operand::V(V::ALL[n]),
                    Class::Special => Operand::Special(Special::ALL[n]),
                    Class::Status => Operand::Status(Status::ALL[n]),
                }
            }
            vun_inline::Operand::Imm(n) => Operand::Imm(*n),
            vun_inline::Operand::Cond(c) => Operand::Cond(Cc::ALL[usize::from(c.0)]),
            vun_inline::Operand::Choice(k) => match kind {
                Kind::Mask => Operand::Mask(Mask::ALL[usize::from(*k)]),
                _ => Operand::Barrier(Barrier::ALL[usize::from(*k)]),
            },
            vun_inline::Operand::Label(line) => place.line(*line),
            vun_inline::Operand::Symbol(s) => match kind {
                Kind::Low => Operand::Low(*s),
                Kind::High => Operand::High(*s),
                _ => Operand::Sym(*s),
            },
            vun_inline::Operand::Bound(_) => Operand::Reg(place.bound()),
            vun_inline::Operand::Mem(mem) => {
                let mut register = |base: &vun_inline::Base<Vb, P>| match base {
                    vun_inline::Base::Reg(r) => Self::general(*r).expect("a general register of an address"),
                    vun_inline::Base::Bound(_) => place.bound(),
                };
                let base = register(mem.base.as_ref().expect("an address with a base"));
                let Kind::Mem(mode, _) = kind else { panic!("memory where the form takes none") };
                Operand::Mem(match (mode, &mem.disp, &mem.index) {
                    (Mode::Index | Mode::NarrowIndex, _, Some((index, _))) => Mem::index(base, register(index)),
                    (Mode::BytePre | Mode::PairPre, &Disp::Bytes(offset), _) => Mem::Pre { base, offset },
                    (Mode::BytePost | Mode::PairPost, &Disp::Bytes(offset), _) => Mem::Post { base, offset },
                    (_, &Disp::Bytes(offset), _) => Mem::Off { base, offset },
                    _ => panic!("memory Thumb does not address"),
                })
            }
        }
    }
}
