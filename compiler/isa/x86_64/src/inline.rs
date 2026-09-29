//! Blocks of inline assembly (spec 07, section 6) read as this assembler's instructions: a block
//! names instructions, forms, widths, conditions and register classes by their index in the
//! table, whose declaration order `generated` keeps. Bindings are general registers, a shift's
//! count `cl`; memory is off a base register or a symbol, never indexed.

use vun_inline::{ClassId, Disp, FormId, Inst, OpId, RegRef, SizeId};
use vun_isa::{AsmOperand, Blocks, Fit, Fixed, Given, Label, Misfit, Place};

use crate::{Access, Base, Cc, Class, Form, Kind, Mem, Op, Operand, Reg, Size, X86_64, Xmm};

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

impl X86_64 {
    /// A register as a block names it, as the assembler takes it.
    #[must_use]
    pub fn register<S>(r: RegRef) -> Operand<S> {
        Self::general(r).map_or(Operand::Xmm(Xmm(r.number)), Operand::Reg)
    }
}

impl Blocks for X86_64 {
    const GENERAL: ClassId = Class::General.id();
    const ARCH: vun_items::Item = vun_items::lang::arch::x86_64;
    type Reg = Reg;
    type Size = Size;
    type Kind = Kind;
    type Operand<S: Copy> = Operand<S>;

    fn kinds<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Kind> {
        Op::entry(inst.op, inst.form).1.kinds()
    }

    fn accesses<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Access> {
        Op::entry(inst.op, inst.form).1.slots.iter().map(|s| s.access)
    }

    fn implicit<V, P, S>(inst: &Inst<V, P, S>) -> impl Iterator<Item = Reg> {
        Op::entry(inst.op, inst.form).1.implicit.iter().copied()
    }

    fn written<V, P, S>(inst: &Inst<V, P, S>) -> Option<(usize, u32)> {
        let form = Op::entry(inst.op, inst.form).1;
        form.slots
            .iter()
            .zip(&inst.operands)
            .position(|(slot, operand)| {
                matches!(operand, vun_inline::Operand::Mem(_))
                    && matches!(slot.access, Access::Write | Access::ReadWrite)
            })
            .and_then(|i| Some((i, form.slots[i].kind.width(Size::of(inst.size))?.bytes())))
    }

    fn general(r: RegRef) -> Option<Reg> {
        match Class::ALL[usize::from(r.class.0)] {
            Class::General => Some(Reg::ALL[usize::from(r.number)]),
            Class::Sse => None,
        }
    }

    fn reference(r: Reg) -> RegRef {
        RegRef { class: Self::GENERAL, number: r as u8 }
    }

    fn binding(kind: Option<Kind>) -> Result<(ClassId, Option<Reg>), AsmOperand> {
        match kind {
            Some(Kind::Cl) => Ok((Self::GENERAL, Some(Reg::Rcx))),
            Some(Kind::R | Kind::Rn(_) | Kind::Rm | Kind::Rmn(_)) | None => Ok((Self::GENERAL, None)),
            Some(Kind::X | Kind::Xmn(_)) => Ok((Class::Sse.id(), None)),
            Some(_) => Err(AsmOperand::Unallocated),
        }
    }

    fn addressable<V, P, S>(mem: &vun_inline::Mem<V, P, S>, _: Kind) -> Result<(), AsmOperand> {
        let fits = |n: i64| i32::try_from(n).map(drop).map_err(|_| AsmOperand::Absolute);
        match (&mem.base, &mem.index, &mem.disp) {
            (_, Some(_), _) => Err(AsmOperand::Indexed),
            (Some(_), None, Disp::Symbol(..)) => Err(AsmOperand::SymbolOffRegister),
            (None, None, Disp::Bytes(_)) => Err(AsmOperand::Absolute),
            (Some(_), None, Disp::Bytes(n)) | (None, None, Disp::Symbol(_, n)) => fits(*n),
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
            vun_inline::Operand::Reg(r) => Self::register(*r),
            vun_inline::Operand::Imm(n) => Operand::Imm(*n),
            vun_inline::Operand::Cond(c) => Operand::Cond(Cc::ALL[usize::from(c.0)]),
            vun_inline::Operand::Choice(_) => panic!("no x86-64 form takes a named choice"),
            vun_inline::Operand::List(_) => panic!("no x86-64 form takes a list"),
            vun_inline::Operand::Label(line) => place.line(*line),
            vun_inline::Operand::Symbol(s) => Operand::Sym(*s),
            vun_inline::Operand::Bound(_) => Self::register(Self::bound(Some(kind), place)),
            vun_inline::Operand::Mem(mem) => {
                let (base, disp) = match (&mem.base, &mem.index, &mem.disp) {
                    (Some(vun_inline::Base::Reg(r)), None, &Disp::Bytes(n)) => {
                        (Base::Reg(Self::general(*r).expect("a general register of an address")), n)
                    }
                    (Some(vun_inline::Base::Bound(_)), None, &Disp::Bytes(n)) => {
                        (Base::Reg(Self::address(place)), n)
                    }
                    (None, None, &Disp::Symbol(s, n)) => (Base::Rip(s, place.via(s)), n),
                    _ => panic!("memory x86-64 does not address"),
                };
                Operand::Mem(Mem { base, disp: i32::try_from(disp).expect("a displacement of 32 bits") })
            }
        }
    }
}
