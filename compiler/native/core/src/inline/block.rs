//! A block emitted into a function: its bindings placed, general ones where the plan puts them
//! and floating-point ones moved or loaded into registers the block leaves free, then its
//! instructions, then its outputs stored.

use vun_asm::{Blocks, Label, Registered, Use};
use vun_backend::{AsmOperand, Lack, Missing};
use vun_codegen::{Bank, Cx, Need, Needs, RegSet};
use vun_inline::{Bound, RegRef};
use vun_lir::{Address, Asm, AsmCheck, Output, Value};

use super::{Inline, Inlined, Placed, Size, sized};

/// A binding of a block as the native backend reads it.
type Used<'a, A> = Use<'a, <A as Inline>::Table, Value, Output, Address>;

impl Inlined for Asm {
    fn check<A: Inline>(&self) -> Result<(), Lack> {
        // A block of another instruction set than the table's.
        if let Some(first) = self.insts.first()
            && self.arch != A::Table::ARCH
        {
            return Err(Missing::Asm.at(first.origin));
        }
        for inst in &self.insts {
            let lack = |o| Missing::AsmOperand(o).at(inst.origin);
            for (kind, operand) in A::Table::kinds(inst).zip(&inst.operands) {
                if let vun_inline::Operand::Mem(mem) = operand {
                    A::addressing(mem, kind).map_err(lack)?;
                }
            }
        }
        for used in A::Table::uses(self) {
            used.fixed().map_err(|o| Missing::AsmOperand(o).at(used.inst.origin))?;
        }
        Ok(())
    }

    fn needs<A: Inline>(&self, check: Option<&AsmCheck>, cx: &Cx<'_, A>, needs: &mut Needs) {
        if check.is_some() {
            needs.temps.push(Bank::General);
        }
        let mut reserved = A::Table::named(self).fold(RegSet::EMPTY, |set, r| set.with(A::preg(r)));
        let mut inputs = RegSet::EMPTY;
        for used in A::Table::uses(self) {
            let held = used.class().is_ok_and(|c| c == A::Table::GENERAL);
            let fixed = used.fixed().ok().flatten().and_then(A::Table::general).map(A::preg);
            match (used.bound, fixed) {
                // A floating-point register's value comes from a general one, rebuilt in a
                // scratch one where it is recomputed.
                (Bound::In { value, .. }, _) if !held => match cx.recomputed(*value) {
                    Some(_) => needs.temps.push(Bank::General),
                    None => needs.read(*value, Need::Reg),
                },
                (Bound::In { value, .. }, _) if cx.recomputed(*value).is_some() => match fixed {
                    Some(p) => reserved = reserved.with(p),
                    None => needs.temps.push(Bank::General),
                },
                (Bound::In { value, .. }, Some(p)) => {
                    needs.read(*value, Need::Fixed(p));
                    inputs = inputs.with(p);
                }
                (Bound::In { value, .. }, None) => needs.read(*value, Need::Reg),
                (Bound::Out { place, .. } | Bound::InOut { place, .. }, _) => {
                    if cx.recomputed(place.addr).is_some() {
                        needs.temps.push(Bank::General);
                    } else {
                        needs.read(place.addr, Need::Reg);
                    }
                    match fixed {
                        Some(p) => reserved = reserved.with(p),
                        None if held => needs.temps.push(Bank::General),
                        None => {}
                    }
                }
            }
        }
        needs.reserved = reserved.without(inputs);
        // What the block leaves in a register it names is its own, an input's included: no value
        // is kept there past it.
        needs.clobbers = needs.clobbers.union(reserved);
    }

    fn emit<A: Inline>(&self, check: Option<&AsmCheck>, cx: &mut Cx<'_, A>) -> Result<(), Lack> {
        let before = check.map(|_| {
            let r = cx.scratch();
            A::stack(&mut cx.asm, r);
            r
        });
        let uses: Vec<Used<'_, A>> = A::Table::uses(self).collect();
        let fixed: Vec<Option<RegRef>> = uses.iter().map(|u| u.fixed().ok().flatten()).collect();
        // The registers the block names of other classes than the general one, which its
        // floating-point bindings leave, and a checked write keeps.
        let mut floats: Vec<RegRef> = Vec::new();
        let named = self.insts.iter().flat_map(|inst| &inst.operands).filter_map(|o| match o {
            vun_inline::Operand::Reg(r) if A::Table::general(*r).is_none() => Some(*r),
            _ => None,
        });
        for r in named.chain(fixed.iter().flatten().copied().filter(|r| A::Table::general(*r).is_none())) {
            if !floats.contains(&r) {
                floats.push(r);
            }
        }
        let free: Vec<RegRef> = A::floats(cx).into_iter().filter(|r| !floats.contains(r)).collect();
        let mut free = free.into_iter();
        let mut regs = Vec::new();
        let mut outputs = Vec::new();
        for (used, fixed) in uses.iter().zip(fixed) {
            let binding = Binding { used, fixed };
            let r = if used.class().is_ok_and(|c| c == A::Table::GENERAL) {
                binding.general(cx, &mut outputs)?
            } else {
                let r = binding.float(cx, &mut free, &mut outputs)?;
                floats.push(r);
                r
            };
            regs.push(r);
        }
        let numbers: Vec<u8> = floats.iter().map(|r| r.number).collect();
        let labels: Vec<Label> = (0..=self.insts.len()).map(|_| cx.asm.label()).collect();
        let mut placed = Placed { regs: regs.into_iter(), labels: &labels };
        for (inst, &label) in self.insts.iter().zip(&labels) {
            cx.asm.bind(label);
            let operands = A::Table::kinds(inst)
                .zip(&inst.operands)
                .map(|(kind, o)| A::Table::operand(o, kind, &mut placed));
            let operands: Vec<_> = operands.collect();
            if let (Some(check), Some((at, bytes))) = (check, A::Table::written(inst)) {
                A::check_write(&mut cx.asm, &operands[at], bytes, &numbers, check);
            }
            A::emit(&mut cx.asm, inst, &operands)
                .map_err(|vun_asm::Untaken| Missing::AsmOperand(AsmOperand::Unencodable).at(inst.origin))?;
        }
        cx.asm.bind(labels[self.insts.len()]);
        if self.returns {
            for (r, addr, size) in outputs {
                A::store_at(&mut cx.asm, addr, r, size);
            }
            if let (Some(check), Some(before)) = (check, before) {
                A::check_stack(&mut cx.asm, before, check);
            }
        }
        Ok(())
    }
}

/// A binding of a block, and the register it must be in, if one.
struct Binding<'u, 'a, A: Inline> {
    used: &'u Used<'a, A>,
    fixed: Option<RegRef>,
}

/// An output's register, the address it is stored at once the block ends, and its width.
type Stored<A> = (RegRef, <A as vun_codegen::Arch>::Reg, Size<A>);

impl<A: Inline> Binding<'_, '_, A> {
    /// Its general register: an input where the plan keeps it or rebuilds it, an output a
    /// register of its own, loaded first for `inout`.
    fn general(&self, cx: &mut Cx<'_, A>, outputs: &mut Vec<Stored<A>>) -> Result<RegRef, Lack> {
        let fixed = self.fixed.and_then(A::Table::general);
        let r = match self.used.bound {
            Bound::In { value, .. } => match fixed {
                Some(r) if cx.recomputed(*value).is_some() => {
                    cx.reg_to(*value, r);
                    r
                }
                // The plan copies it into its register before the block, wherever it keeps it.
                Some(r) => r,
                None => cx.reg(*value),
            },
            Bound::Out { .. } | Bound::InOut { .. } => fixed.unwrap_or_else(|| cx.scratch()),
        };
        let r = A::Table::reference(r);
        self.output(r, cx, outputs)?;
        Ok(r)
    }

    /// Its floating-point register, the one it names or the next `free`: an input moved there
    /// from its general register.
    fn float(
        &self,
        cx: &mut Cx<'_, A>,
        free: &mut impl Iterator<Item = RegRef>,
        outputs: &mut Vec<Stored<A>>,
    ) -> Result<RegRef, Lack> {
        let crowded = || Missing::AsmOperand(AsmOperand::Crowded).at(self.used.inst.origin);
        let r = self.fixed.or_else(|| free.next()).ok_or_else(crowded)?;
        if let Bound::In { value, .. } = self.used.bound {
            let size = sized::<Size<A>>(cx.f.ty(*value), cx.unit.pointer_bits, self.used.inst)?;
            let src = cx.reg(*value);
            A::moved(&mut cx.asm, r, src, size);
        }
        self.output(r, cx, outputs)?;
        Ok(r)
    }

    /// Where it is an output: `r` loaded first for `inout`, and stored once the block ends.
    fn output(&self, r: RegRef, cx: &mut Cx<'_, A>, outputs: &mut Vec<Stored<A>>) -> Result<(), Lack> {
        let (Bound::Out { place, .. } | Bound::InOut { place, .. }) = self.used.bound else { return Ok(()) };
        let (addr, size) =
            (cx.reg(place.addr), sized::<Size<A>>(place.ty, cx.unit.pointer_bits, self.used.inst)?);
        if matches!(self.used.bound, Bound::InOut { .. }) {
            A::load_at(&mut cx.asm, r, addr, size);
        }
        outputs.push((r, addr, size));
        Ok(())
    }
}
