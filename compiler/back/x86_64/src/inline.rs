//! Blocks of inline assembly (spec 07, section 6) read as this assembler's instructions: a block
//! names instructions, forms, widths, conditions and register classes by their index in the
//! table, whose declaration order `generated` keeps.

use vun_asm::Via;
use vun_inline::{Disp, FormId, Line, OpId, RegRef, SizeId};

use crate::{Base, Cc, Class, Form, Mem, Op, Operand, Reg, Size, Xmm};

/// Where a block's operands are, as the code it is emitted into has them.
pub trait Place<S> {
    /// The register of the next binding, in the order `Block::bindings` gives them.
    fn bound(&mut self) -> Reg;

    /// The operand naming the start of `line`.
    fn line(&self, line: Line) -> Operand<S>;

    /// How an address reaches `symbol`.
    fn via(&self, symbol: S) -> Via;
}

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

impl Reg {
    /// The general register a block names, if it names one.
    #[must_use]
    pub const fn named(r: RegRef) -> Option<Self> {
        match Class::ALL[r.class.0 as usize] {
            Class::General => Some(Self::ALL[r.number as usize]),
            Class::Sse => None,
        }
    }
}

impl<S: Copy> Operand<S> {
    /// A block's operand as this assembler takes it, placed where `place` says.
    ///
    /// # Panics
    /// Panics on memory x86-64 does not address: indexed, off a register and a symbol, or with
    /// a displacement past 32 bits; a block is checked for those first.
    pub fn of<V, P>(operand: &vun_inline::Operand<V, P, S>, place: &mut impl Place<S>) -> Self {
        match operand {
            vun_inline::Operand::Reg(r) => Reg::named(*r).map_or(Self::Xmm(Xmm(r.number)), Self::Reg),
            vun_inline::Operand::Imm(n) => Self::Imm(*n),
            vun_inline::Operand::Cond(c) => Self::Cond(Cc::ALL[usize::from(c.0)]),
            vun_inline::Operand::Label(line) => place.line(*line),
            vun_inline::Operand::Symbol(s) => Self::Sym(*s),
            vun_inline::Operand::Bound(_) => Self::Reg(place.bound()),
            vun_inline::Operand::Mem(mem) => {
                let (base, disp) = match (&mem.base, &mem.index, &mem.disp) {
                    (Some(vun_inline::Base::Reg(r)), None, &Disp::Bytes(n)) => {
                        (Base::Reg(Reg::named(*r).expect("a general register of an address")), n)
                    }
                    (Some(vun_inline::Base::Bound(_)), None, &Disp::Bytes(n)) => {
                        (Base::Reg(place.bound()), n)
                    }
                    (None, None, &Disp::Symbol(s, n)) => (Base::Rip(s, place.via(s)), n),
                    _ => panic!("memory x86-64 does not address"),
                };
                Self::Mem(Mem { base, disp: i32::try_from(disp).expect("a displacement of 32 bits") })
            }
        }
    }
}
