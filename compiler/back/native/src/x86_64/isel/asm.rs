//! Inline assembly (spec 07, section 6): each instruction encoded by the form it was checked
//! against, over the registers the plan gives its bindings. Every register the block names or
//! its forms read or write unnamed is reserved for the whole block, so no binding is held there;
//! an output is written in a register of its own, loaded first for `inout`, and stored through
//! its address once the block ends.

use vun_asm::{Label, Via};
use vun_asm_x86_64::{Base, Cc, Class, Emit, Form, Kind, Mem, Op, Operand, Reg, Size, Xmm};
use vun_backend::{AsmOperand, Lack, Missing};
use vun_codegen::{Arch, Bank, Need, Needs, RegSet};
use vun_inline::{Bound, Disp, RegRef};
use vun_lir::{Address, Asm, Output, Value};

use super::Sel;
use crate::x86_64::X86;

type Inst = vun_inline::Inst<Value, Output, Address>;
type AsmOperandOf = vun_inline::Operand<Value, Output, Address>;
type Binding = Bound<Value, Output>;

/// A binding of a block, with its instruction and the kind of the position it is at: `None`
/// for an address's base.
pub(super) struct Use<'a> {
    inst: &'a Inst,
    kind: Option<Kind>,
    bound: &'a Binding,
}

impl Use<'_> {
    /// The register it must be in, if one: the one it names, or `cl`'s for a shift's count.
    /// Only general registers hold values.
    fn fixed(&self) -> Result<Option<Reg>, AsmOperand> {
        let named = self.bound.reg().map(|r| Named::of(r).general()).transpose()?;
        match self.kind {
            Some(Kind::Cl) => Ok(Some(Reg::Rcx)),
            Some(Kind::R | Kind::Rn(_) | Kind::Rm | Kind::Rmn(_)) | None => Ok(named),
            Some(_) => Err(AsmOperand::Unallocated),
        }
    }
}

/// A block of inline assembly as x86-64 encodes it.
pub(super) trait Encoded {
    /// Its bindings, in the order `Block::bindings` gives them.
    fn uses(&self) -> impl Iterator<Item = Use<'_>>;

    /// Whether every memory operand is addressed as x86-64 can, and every binding has a
    /// register.
    fn check(&self) -> Result<(), Lack>;

    /// Its inputs in registers, the addresses of its outputs, a register for each output, and
    /// the registers it names or its forms use unnamed kept for it, but where an input is read.
    fn needs(&self, needs: &mut Needs);
}

impl Encoded for Asm {
    fn uses(&self) -> impl Iterator<Item = Use<'_>> {
        self.insts.iter().flat_map(|inst| {
            let form = inst.form();
            form.slots.iter().zip(&inst.operands).filter_map(move |(slot, operand)| {
                let (kind, bound) = match operand {
                    vun_inline::Operand::Bound(b) => (Some(slot.kind), b),
                    vun_inline::Operand::Mem(vun_inline::Mem { base: Some(vun_inline::Base::Bound(b)), .. }) => {
                        (None, b)
                    }
                    _ => return None,
                };
                Some(Use { inst, kind, bound })
            })
        })
    }

    fn check(&self) -> Result<(), Lack> {
        for inst in &self.insts {
            for operand in &inst.operands {
                if let vun_inline::Operand::Mem(mem) = operand {
                    mem.addressing().map_err(|o| Missing::AsmOperand(o).at(inst.origin))?;
                }
            }
        }
        for used in self.uses() {
            used.fixed().map_err(|o| Missing::AsmOperand(o).at(used.inst.origin))?;
        }
        Ok(())
    }

    fn needs(&self, needs: &mut Needs) {
        let mut reserved = RegSet::EMPTY;
        let mut inputs = RegSet::EMPTY;
        for inst in &self.insts {
            let named = inst.operands.iter().filter_map(|operand| match operand {
                vun_inline::Operand::Reg(r)
                | vun_inline::Operand::Mem(vun_inline::Mem { base: Some(vun_inline::Base::Reg(r)), .. }) => {
                    Named::of(*r).general().ok()
                }
                _ => None,
            });
            let regs = inst.form().implicit.iter().copied().chain(named);
            reserved = regs.fold(reserved, |set, r| set.with(X86::preg(r)));
        }
        for used in self.uses() {
            let fixed = used.fixed().ok().flatten().map(X86::preg);
            match used.bound {
                Bound::In { value, .. } => {
                    needs.read(*value, fixed.map_or(Need::Reg, Need::Fixed));
                    inputs = fixed.map_or(inputs, |p| inputs.with(p));
                }
                Bound::Out { place, .. } | Bound::InOut { place, .. } => {
                    needs.read(place.addr, Need::Reg);
                    match fixed {
                        Some(p) => reserved = reserved.with(p),
                        None => needs.temps.push(Bank::General),
                    }
                }
            }
        }
        needs.reserved = reserved.without(inputs);
    }
}

/// An instruction of a block, by its entry in the x86-64 table.
trait Entry {
    fn op(&self) -> Op;

    /// The form it was checked against.
    fn form(&self) -> &'static Form;
}

impl Entry for Inst {
    fn op(&self) -> Op {
        Op::ALL[usize::from(self.op.0)]
    }

    fn form(&self) -> &'static Form {
        &Entry::op(self).forms()[usize::from(self.form.0)]
    }
}

/// A register the block names, of either class.
#[derive(Clone, Copy)]
enum Named {
    Reg(Reg),
    Xmm(Xmm),
}

impl Named {
    fn of(r: RegRef) -> Self {
        match Class::ALL[usize::from(r.class.0)] {
            Class::General => Self::Reg(Reg::ALL[usize::from(r.number)]),
            Class::Sse => Self::Xmm(Xmm(r.number)),
        }
    }

    fn general(self) -> Result<Reg, AsmOperand> {
        match self {
            Self::Reg(r) => Ok(r),
            Self::Xmm(_) => Err(AsmOperand::Unallocated),
        }
    }
}

/// A memory operand of a block, as x86-64 addresses it.
trait Addressing {
    /// Off a base register or a symbol this module defines, never indexed, the displacement 32
    /// bits.
    fn addressing(&self) -> Result<(), AsmOperand>;
}

impl Addressing for vun_inline::Mem<Value, Output, Address> {
    fn addressing(&self) -> Result<(), AsmOperand> {
        let fits = |n: i64| i32::try_from(n).map(drop).map_err(|_| AsmOperand::Absolute);
        match (&self.base, &self.index, &self.disp) {
            (_, Some(_), _) => Err(AsmOperand::Indexed),
            (Some(_), None, Disp::Symbol(..)) => Err(AsmOperand::SymbolOffRegister),
            (None, None, Disp::Bytes(_)) => Err(AsmOperand::Absolute),
            (None, None, Disp::Symbol(Address::Import(_), _)) => Err(AsmOperand::Imported),
            (Some(_), None, Disp::Bytes(n)) | (None, None, Disp::Symbol(_, n)) => fits(*n),
        }
    }
}

impl Sel<'_, '_> {
    /// Emits `asm`: each output given its register, loaded for `inout`; each instruction by its
    /// form; then each output stored, where the block goes on.
    pub(super) fn asm(&mut self, asm: &Asm) -> Result<(), Lack> {
        let mut regs = Vec::new();
        let mut outputs = Vec::new();
        for used in asm.uses() {
            let r = match used.bound {
                Bound::In { value, .. } => self.reg(*value),
                Bound::Out { place, .. } | Bound::InOut { place, .. } => {
                    let r = match used.fixed() {
                        Ok(Some(r)) => r,
                        _ => self.scratch(),
                    };
                    let (addr, size) = (self.reg(place.addr), self.size_of(place.ty)?);
                    if matches!(used.bound, Bound::InOut { .. }) {
                        self.asm.emit(Op::Mov, size, &[r.into(), Mem::at(addr, 0).into()]);
                    }
                    outputs.push((r, addr, size));
                    r
                }
            };
            regs.push(r);
        }
        let labels: Vec<Label> = (0..=asm.insts.len()).map(|_| self.asm.label()).collect();
        let mut regs = regs.into_iter();
        for (inst, &label) in asm.insts.iter().zip(&labels) {
            self.asm.bind(label);
            let operands: Vec<_> = inst.operands.iter().map(|o| operand(o, &mut regs, &labels)).collect();
            self.asm.emit_by(Entry::op(inst), inst.form(), Size::ALL[usize::from(inst.size.0)], &operands);
        }
        self.asm.bind(labels[asm.insts.len()]);
        if asm.returns {
            for (r, addr, size) in outputs {
                self.asm.emit(Op::Mov, size, &[Mem::at(addr, 0).into(), r.into()]);
            }
        }
        Ok(())
    }
}

/// An operand as x86-64 encodes it, its bindings taking `regs` in order and its lines
/// `labels`, of shapes `Encoded::check` accepted.
fn operand(operand: &AsmOperandOf, regs: &mut impl Iterator<Item = Reg>, labels: &[Label]) -> Operand<Address> {
    let mut next = || regs.next().expect("a register for each binding");
    match operand {
        vun_inline::Operand::Reg(r) => match Named::of(*r) {
            Named::Reg(r) => Operand::Reg(r),
            Named::Xmm(x) => Operand::Xmm(x),
        },
        vun_inline::Operand::Imm(n) => Operand::Imm(*n),
        vun_inline::Operand::Cond(c) => Operand::Cond(Cc::ALL[usize::from(c.0)]),
        vun_inline::Operand::Label(line) => Operand::Label(labels[usize::from(line.0)]),
        vun_inline::Operand::Symbol(s) => Operand::Sym(*s),
        vun_inline::Operand::Bound(_) => Operand::Reg(next()),
        vun_inline::Operand::Mem(mem) => {
            let (base, disp) = match (&mem.base, mem.disp.clone()) {
                (Some(vun_inline::Base::Reg(r)), Disp::Bytes(n)) => {
                    (Base::Reg(Named::of(*r).general().expect("a general base")), n)
                }
                (Some(vun_inline::Base::Bound(_)), Disp::Bytes(n)) => (Base::Reg(next()), n),
                (None, Disp::Symbol(s, n)) => (Base::Rip(s, Via::Direct), n),
                _ => unreachable!("an address x86-64 encodes"),
            };
            Operand::Mem(Mem { base, disp: i32::try_from(disp).expect("a displacement of 32 bits") })
        }
    }
}
