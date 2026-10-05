//! The registers a block's bindings are held in, as the C statement declares them: each the one
//! it names or its position takes, else a general one of those a call of the C convention
//! overwrites, its arguments' first, or a floating-point one of those it passes floats in.

use vun_asm::{AsmOperand, Registered, Use, Width};
use vun_backend::{Lack, Missing};
use vun_inline::{Bound, RegRef};
use vun_lir::{Output, Scalar, Value};

use super::FnEmit;
use super::gnu::{Block, Gnu, Named};

/// A binding's register: a general one, holding an address's word, or a floating-point one,
/// holding its float at a width.
pub(super) enum Held<I: Registered> {
    General(I::Reg),
    Float { reg: RegRef, ty: Scalar, size: I::Size },
}

impl<I: Registered> Held<I> {
    /// The register, as a block names it.
    pub(super) fn reg(&self) -> RegRef {
        match *self {
            Self::General(r) => I::reference(r),
            Self::Float { reg, .. } => reg,
        }
    }
}

/// The registers of a block's bindings, in order, and the general ones it leaves free.
pub(super) struct Registers<I: Registered> {
    pub(super) held: Vec<Held<I>>,
    pub(super) free: std::vec::IntoIter<I::Reg>,
}

impl FnEmit<'_, '_> {
    /// The registers of `uses`, the bindings of `block`.
    pub(super) fn registers<I: Gnu>(
        &self,
        block: &Block,
        uses: &[Use<'_, I, Value, Output, Named>],
    ) -> Result<Registers<I>, Lack> {
        let convention = self.e.target.convention;
        let mut fixed = Vec::with_capacity(uses.len());
        for used in uses {
            fixed.push(used.fixed().map_err(|o| Missing::AsmOperand(o).at(used.inst.origin))?);
        }
        // Every register the block names or a binding is fixed to, of any class.
        let named = block.insts.iter().flat_map(|inst| &inst.operands).filter_map(|o| match o {
            vun_inline::Operand::Reg(r) => Some(*r),
            _ => None,
        });
        let taken: Vec<RegRef> = named.chain(fixed.iter().flatten().copied()).collect();
        let implicit: Vec<I::Reg> = I::named(block).collect();
        let free: Vec<I::Reg> = I::free(convention)
            .filter(|r| !implicit.contains(r) && !taken.contains(&I::reference(*r)))
            .collect();
        let mut free = free.into_iter();
        let mut floats = I::floats(convention).into_iter().filter(|r| !taken.contains(r));
        let mut held = Vec::with_capacity(uses.len());
        for (used, fixed) in uses.iter().zip(fixed) {
            let lack = |o| Missing::AsmOperand(o).at(used.inst.origin);
            if used.class().map_err(lack)? == I::GENERAL {
                let reg = fixed.and_then(I::general).or_else(|| free.next());
                held.push(Held::General(reg.ok_or_else(|| lack(AsmOperand::Crowded))?));
                continue;
            }
            let ty = match used.bound {
                Bound::In { value, .. } => self.func.ty(*value),
                Bound::Out { place, .. } | Bound::InOut { place, .. } => place.ty,
            };
            let size = I::Size::of_bytes(ty.bytes(self.e.unit.pointer_bits))
                .ok_or_else(|| lack(AsmOperand::Unallocated))?;
            let reg = fixed.or_else(|| floats.by_ref().find(|r| I::float_name(r.number, size).is_some()));
            held.push(Held::Float { reg: reg.ok_or_else(|| lack(AsmOperand::Crowded))?, ty, size });
        }
        Ok(Registers { held, free })
    }
}
