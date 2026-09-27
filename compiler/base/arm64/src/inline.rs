//! Blocks of inline assembly (spec 07, section 6) read as this assembler's instructions: a block
//! names instructions, forms, widths, conditions and register classes by their index in the
//! table, whose declaration order `generated` keeps. Bindings are general registers; memory is in
//! the addressing mode its form names, and a symbol a branch's target, a page or its low bits.

use vun_inline::{Disp, FormId, Inst, OpId, RegRef, SizeId};
use vun_isa::{AsmOperand, Blocks, Fit, Fixed, Given, Label, Misfit, Place};

use crate::{
    Access, Arm64, Barrier, Cc, Class, Extend, Form, Kind, Mem, Mode, Op, Operand, Reg, Scale, Size, SysReg,
    V,
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

impl Blocks for Arm64 {
    const ARCH: vun_items::Item = vun_items::lang::arch::arm64;
    type Reg = Reg;
    type Kind = Kind;
    type Operand<S: Copy> = Operand<S>;

    fn kinds<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Kind> {
        Op::entry(inst.op, inst.form).1.args.iter().map(|arg| arg.kind)
    }

    fn implicit<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Reg> {
        let form = Op::entry(inst.op, inst.form).1;
        form.reads.iter().chain(form.writes).copied()
    }

    /// A pair writes two of its access's width.
    fn written<V, P, S>(inst: &Inst<V, P, S>) -> Option<(usize, u32)> {
        let form = Op::entry(inst.op, inst.form).1;
        form.args.iter().enumerate().find_map(|(i, arg)| match (arg.kind, arg.access) {
            (Kind::Mem(mode, width), Access::Write | Access::ReadWrite) => {
                let pairs = if matches!(mode, Mode::Pair | Mode::PairPre | Mode::PairPost) { 2 } else { 1 };
                Some((i, width.at(Size::of(inst.size)).bytes() * pairs))
            }
            _ => None,
        })
    }

    fn general(r: RegRef) -> Option<Reg> {
        match Class::ALL[usize::from(r.class.0)] {
            Class::General => Some(Reg::ALL[usize::from(r.number)]),
            Class::Fp => None,
        }
    }

    fn fixed(kind: Option<Kind>, named: Option<Reg>) -> Result<Option<Reg>, AsmOperand> {
        match kind {
            Some(Kind::Gpr(..) | Kind::Shifted(_) | Kind::Extended) | None => Ok(named),
            Some(_) => Err(AsmOperand::Unallocated),
        }
    }

    /// Off a base register, indexed unscaled in `index`, a symbol's low bits in `lo12`.
    fn addressable<V, P, S>(mem: &vun_inline::Mem<V, P, S>, kind: Kind) -> Result<(), AsmOperand> {
        let Kind::Mem(mode, _) = kind else { return Err(AsmOperand::Unencodable) };
        match (mode, &mem.base, &mem.index, &mem.disp) {
            (_, None, _, _) => Err(AsmOperand::Absolute),
            (Mode::Index, Some(_), Some((_, 1)), Disp::Bytes(0))
            | (Mode::Lo12, Some(_), None, Disp::Symbol(_, 0))
            | (_, Some(_), None, Disp::Bytes(_)) => Ok(()),
            (Mode::Index | Mode::Lo12, ..) => Err(AsmOperand::Unencodable),
            (_, Some(_), Some(_), _) => Err(AsmOperand::Indexed),
            (_, Some(_), None, Disp::Symbol(..)) => Err(AsmOperand::SymbolOffRegister),
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

    fn operand<V, P, S: Copy>(
        operand: &vun_inline::Operand<V, P, S>,
        kind: Kind,
        place: &mut impl Place<Self, S>,
    ) -> Operand<S> {
        match operand {
            vun_inline::Operand::Reg(r) => Self::general(*r).map_or(Operand::V(V(r.number)), Operand::Reg),
            vun_inline::Operand::Imm(n) => Operand::Imm(*n),
            vun_inline::Operand::Cond(c) => Operand::Cond(Cc::ALL[usize::from(c.0)]),
            vun_inline::Operand::Choice(k) => match kind {
                Kind::Sys => Operand::Sys(SysReg::ALL[usize::from(*k)]),
                _ => Operand::Barrier(Barrier::ALL[usize::from(*k)]),
            },
            vun_inline::Operand::Label(line) => place.line(*line),
            vun_inline::Operand::Symbol(s) => match kind {
                Kind::Page => Operand::Page(*s, place.via(*s)),
                Kind::Lo12 => Operand::Lo12(*s),
                _ => Operand::Sym(*s),
            },
            vun_inline::Operand::Bound(_) => Operand::Reg(place.bound()),
            vun_inline::Operand::Mem(mem) => {
                let mut register = |base: &vun_inline::Base<V, P>| match base {
                    vun_inline::Base::Reg(r) => Self::general(*r).expect("a general register of an address"),
                    vun_inline::Base::Bound(_) => place.bound(),
                };
                let base = register(mem.base.as_ref().expect("an address with a base"));
                let Kind::Mem(mode, _) = kind else { panic!("memory where the form takes none") };
                Operand::Mem(match (mode, &mem.disp, &mem.index) {
                    (Mode::Index, _, Some((index, _))) => {
                        let index = register(index);
                        Mem::Index { base, index, extend: Extend::LSL, scale: Scale::One }
                    }
                    (Mode::Lo12, &Disp::Symbol(target, _), _) => {
                        Mem::Lo12 { base, target, via: place.via(target) }
                    }
                    (Mode::Pre | Mode::PairPre, &Disp::Bytes(offset), _) => Mem::Pre { base, offset },
                    (Mode::Post | Mode::PairPost, &Disp::Bytes(offset), _) => Mem::Post { base, offset },
                    (_, &Disp::Bytes(offset), _) => Mem::Off { base, offset },
                    _ => panic!("memory arm64 does not address"),
                })
            }
        }
    }
}
