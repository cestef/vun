//! Inline assembly (spec 07, section 6) on every native architecture: each instruction encoded
//! by the form it was checked against, over the registers the plan gives its bindings. Every
//! register the block names or its forms use unnamed is reserved for the whole block, so no
//! binding is held there, but for an input read where it names; an output is written in a
//! register of its own, loaded first for `inout`, and stored through its address once the block
//! ends. Where the program checks its blocks (`AsmCheck`), each instruction writing memory first
//! has the address it writes checked, and a block that returns has its stack pointer compared
//! with the one it started with (spec 13, section 8.7). What differs between architectures,
//! their tables and operands, is [`Inline`].

use vun_asm::{Blocks, Label, Place, Untaken, Via};
use vun_backend::{AsmOperand, Lack, Missing};
use vun_codegen::{Bank, Code, Cx, Isel, Need, Needs, RegSet};
use vun_inline::Bound;
use vun_lir::{Address, Asm, AsmCheck, Output, Scalar, Value};

use crate::width::{Width, size_of};

pub(crate) type Inst = vun_inline::Inst<Value, Output, Address>;
pub(crate) type Mem = vun_inline::Mem<Value, Output, Address>;

/// What a native architecture adds to its instruction set's facts about blocks (`Blocks`): how
/// it emits an instruction, and loads and stores an output.
pub(crate) trait Inline: Isel {
    /// The instruction set, as it reads blocks.
    type Table: Blocks<Reg = Self::Reg>;
    type Size: Width;

    /// Whether memory a block names at a position of `kind` is one this code addresses.
    ///
    /// # Errors
    /// What it does not address.
    fn addressing(mem: &Mem, kind: Kind<Self>) -> Result<(), AsmOperand> {
        Self::Table::addressable(mem, kind)
    }

    /// Emits `inst` over `operands` by its form.
    ///
    /// # Errors
    /// The form does not encode them.
    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Encoded<Self>]) -> Result<(), Untaken>;

    /// Checks through `check.write` the `bytes` the next instruction writes at `at`, keeping every
    /// register and the flags as they were.
    ///
    /// # Errors
    /// What the architecture lacks for it.
    fn check_write(
        code: &mut Code<Self>,
        at: &Encoded<Self>,
        bytes: u32,
        check: &AsmCheck,
    ) -> Result<(), Missing>;

    /// Writes the stack pointer into `dst`.
    fn stack(code: &mut Code<Self>, dst: Self::Reg);

    /// Ends the program through `check.stack` unless the stack pointer is `before`.
    fn check_stack(code: &mut Code<Self>, before: Self::Reg, check: &AsmCheck);

    /// Loads `r` from `addr`, or stores it there, `size` wide.
    fn load_at(code: &mut Code<Self>, r: Self::Reg, addr: Self::Reg, size: Self::Size);
    fn store_at(code: &mut Code<Self>, addr: Self::Reg, r: Self::Reg, size: Self::Size);
}

/// What an operand position of `A` takes.
pub(crate) type Kind<A> = <<A as Inline>::Table as Blocks>::Kind;
/// An operand as `A`'s assembler encodes it.
pub(crate) type Encoded<A> = <<A as Inline>::Table as Blocks>::Operand<Address>;

/// Where a block's operands are in the function compiled: its bindings in the registers the
/// plan gives them, in order, and its lines at `labels`.
pub(crate) struct Placed<'a, T: Blocks> {
    regs: std::vec::IntoIter<T::Reg>,
    labels: &'a [Label],
}

impl<T: Blocks> Place<T, Address> for Placed<'_, T> {
    fn bound(&mut self) -> T::Reg {
        self.regs.next().expect("a register for each binding")
    }

    fn line(&self, line: vun_inline::Line) -> T::Operand<Address> {
        T::label(self.labels[usize::from(line.0)])
    }

    /// An import through its entry in the global offset table.
    fn via(&self, target: Address) -> Via {
        match target {
            Address::Import(_) => Via::Got,
            Address::Func(_) | Address::Data(_) => Via::Direct,
        }
    }
}

/// A block of inline assembly, as an architecture encodes it.
pub(crate) trait Inlined {
    /// Whether every memory operand is one `A` addresses, and every binding has a register.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn check<A: Inline>(&self) -> Result<(), Lack>;

    /// Its inputs in registers, the addresses of its outputs, a register for each output, and
    /// the registers it names or its forms use unnamed kept for it, but where an input is read.
    /// A value recomputed where used takes a scratch register, or is written into the one it
    /// is fixed to.
    /// Where it is checked, a register keeps the stack pointer it starts with.
    fn needs<A: Inline>(&self, check: Option<&AsmCheck>, cx: &Cx<'_, A>, needs: &mut Needs);

    /// Emits it: each output given its register, loaded for `inout`; each instruction by its
    /// form; then each output stored, where the block goes on.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn emit<A: Inline>(&self, check: Option<&AsmCheck>, cx: &mut Cx<'_, A>) -> Result<(), Lack>;
}

impl Inlined for Asm {
    fn check<A: Inline>(&self) -> Result<(), Lack> {
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
            let fixed = used.fixed().ok().flatten().map(A::preg);
            match (used.bound, fixed) {
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
                        None => needs.temps.push(Bank::General),
                    }
                }
            }
        }
        needs.reserved = reserved.without(inputs);
    }

    fn emit<A: Inline>(&self, check: Option<&AsmCheck>, cx: &mut Cx<'_, A>) -> Result<(), Lack> {
        let before = check.map(|_| {
            let r = cx.scratch();
            A::stack(&mut cx.asm, r);
            r
        });
        let mut regs = Vec::new();
        let mut outputs = Vec::new();
        for used in A::Table::uses(self) {
            let r = match used.bound {
                Bound::In { value, .. } => match used.fixed() {
                    Ok(Some(r)) if cx.recomputed(*value).is_some() => {
                        cx.reg_to(*value, r);
                        r
                    }
                    _ => cx.reg(*value),
                },
                Bound::Out { place, .. } | Bound::InOut { place, .. } => {
                    let r = match used.fixed() {
                        Ok(Some(r)) => r,
                        _ => cx.scratch(),
                    };
                    let (addr, size) = (cx.reg(place.addr), sized::<A::Size>(place.ty, used.inst)?);
                    if matches!(used.bound, Bound::InOut { .. }) {
                        A::load_at(&mut cx.asm, r, addr, size);
                    }
                    outputs.push((r, addr, size));
                    r
                }
            };
            regs.push(r);
        }
        let labels: Vec<Label> = (0..=self.insts.len()).map(|_| cx.asm.label()).collect();
        let mut placed = Placed::<A::Table> { regs: regs.into_iter(), labels: &labels };
        for (inst, &label) in self.insts.iter().zip(&labels) {
            cx.asm.bind(label);
            let operands = A::Table::kinds(inst)
                .zip(&inst.operands)
                .map(|(kind, o)| A::Table::operand(o, kind, &mut placed));
            let operands: Vec<_> = operands.collect();
            if let (Some(check), Some((at, bytes))) = (check, A::Table::written(inst)) {
                A::check_write(&mut cx.asm, &operands[at], bytes, check).map_err(|m| m.at(inst.origin))?;
            }
            A::emit(&mut cx.asm, inst, &operands)
                .map_err(|Untaken| Missing::AsmOperand(AsmOperand::Unencodable).at(inst.origin))?;
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

/// The size an output of type `ty` is loaded and stored at, or what the backend lacks for it.
fn sized<S: Width>(ty: Scalar, inst: &Inst) -> Result<S, Lack> {
    size_of(ty).ok_or_else(|| {
        let missing = if ty == Scalar::I128 { Missing::Int128 } else { Missing::Half };
        missing.at(inst.origin)
    })
}
