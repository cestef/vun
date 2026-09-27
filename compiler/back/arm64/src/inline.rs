//! Blocks of inline assembly (spec 07, section 6) read as this assembler's instructions: a block
//! names instructions, forms, widths, conditions and register classes by their index in the
//! table, whose declaration order `generated` keeps.

use vun_asm::Via;
use vun_inline::{Disp, FormId, Line, OpId, RegRef, SizeId};

use crate::{Cc, Class, Extend, Form, Kind, Mem, Mode, Op, Operand, Reg, Scale, Size, V};

/// Where a block's operands are, as the code it is emitted into has them.
pub trait Place<S> {
    /// The register of the next binding, in the order `Block::bindings` gives them.
    fn bound(&mut self) -> Reg;

    /// The operand naming the start of `line`.
    fn line(&self, line: Line) -> Operand<S>;

    /// How a page or an address reaches `symbol`.
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
            Class::Fp => None,
        }
    }
}

impl<S: Copy> Operand<S> {
    /// A block's operand at a position of `kind` as this assembler takes it, placed where
    /// `place` says: a symbol as the position takes it, memory in the position's mode.
    ///
    /// # Panics
    /// Panics on memory without a base, or a symbol past a register outside `lo12`; a block is
    /// checked for those first.
    pub fn of<V, P>(operand: &vun_inline::Operand<V, P, S>, kind: Kind, place: &mut impl Place<S>) -> Self {
        match operand {
            vun_inline::Operand::Reg(r) => Reg::named(*r).map_or(Self::V(V(r.number)), Self::Reg),
            vun_inline::Operand::Imm(n) => Self::Imm(*n),
            vun_inline::Operand::Cond(c) => Self::Cond(Cc::ALL[usize::from(c.0)]),
            vun_inline::Operand::Label(line) => place.line(*line),
            vun_inline::Operand::Symbol(s) => match kind {
                Kind::Page => Self::Page(*s, place.via(*s)),
                Kind::Lo12 => Self::Lo12(*s),
                _ => Self::Sym(*s),
            },
            vun_inline::Operand::Bound(_) => Self::Reg(place.bound()),
            vun_inline::Operand::Mem(mem) => {
                let mut register = |base: &vun_inline::Base<V, P>| match base {
                    vun_inline::Base::Reg(r) => Reg::named(*r).expect("a general register of an address"),
                    vun_inline::Base::Bound(_) => place.bound(),
                };
                let base = register(mem.base.as_ref().expect("an address with a base"));
                let Kind::Mem(mode, _) = kind else { panic!("memory where the form takes none") };
                Self::Mem(match (mode, &mem.disp, &mem.index) {
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
