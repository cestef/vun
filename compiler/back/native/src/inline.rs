//! Inline assembly (spec 07, section 6) on every native architecture: each instruction encoded
//! by the form it was checked against, over the registers the plan gives its bindings. Every
//! register the block names or its forms use unnamed is reserved for the whole block, so no
//! binding is held there, but for an input read where it names; an output is written in a
//! register of its own, loaded first for `inout`, and stored through its address once the block
//! ends. What differs between architectures, their tables and operands, is [`Inline`].

use vun_asm::{Label, Untaken, Via};
use vun_backend::{AsmOperand, Lack, Missing};
use vun_codegen::{Bank, Code, Cx, Isel, Need, Needs, RegSet};
use vun_inline::{Bound, RegRef};
use vun_lir::{Address, Asm, Output, Scalar, Value};

use crate::width::{Width, size_of};

pub(crate) type Inst = vun_inline::Inst<Value, Output, Address>;
pub(crate) type Operand = vun_inline::Operand<Value, Output, Address>;
pub(crate) type Mem = vun_inline::Mem<Value, Output, Address>;
type Binding = Bound<Value, Output>;

/// What an architecture tells inline assembly: its table, and its operands.
pub(crate) trait Inline: Isel {
    /// What an operand position takes.
    type Kind: Copy;
    /// An operand as the assembler encodes it.
    type Operand;
    type Size: Width;

    /// The kinds of the positions of the form `inst` was checked against, in order.
    fn kinds(inst: &Inst) -> impl Iterator<Item = Self::Kind>;

    /// The value registers the form reads or writes without naming them.
    fn implicit(inst: &Inst) -> impl Iterator<Item = Self::Reg>;

    /// The register `r` is, if its class is the one values are held in.
    fn general(r: RegRef) -> Option<Self::Reg>;

    /// The register a binding at a position of `kind` must be in, if one: `named`, or the one
    /// the position takes; `None` for an address's base.
    ///
    /// # Errors
    /// The position takes no value register.
    fn fixed(kind: Option<Self::Kind>, named: Option<Self::Reg>) -> Result<Option<Self::Reg>, AsmOperand>;

    /// Whether memory of this shape at a position of `kind` is one the architecture addresses.
    ///
    /// # Errors
    /// What it cannot address.
    fn addressing(mem: &Mem, kind: Self::Kind) -> Result<(), AsmOperand>;

    /// `operand` at a position of `kind` as the assembler takes it, where `placed` says.
    fn operand(operand: &Operand, kind: Self::Kind, placed: &mut Placed<'_, Self::Reg>) -> Self::Operand;

    /// Emits `inst` over `operands` by its form.
    ///
    /// # Errors
    /// The form does not encode them.
    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Self::Operand]) -> Result<(), Untaken>;

    /// Loads `r` from `addr`, or stores it there, `size` wide.
    fn load_at(code: &mut Code<Self>, r: Self::Reg, addr: Self::Reg, size: Self::Size);
    fn store_at(code: &mut Code<Self>, addr: Self::Reg, r: Self::Reg, size: Self::Size);
}

/// Where a block's operands are in the function compiled: its bindings in the registers the
/// plan gives them, in order, and its lines at `labels`.
pub(crate) struct Placed<'a, R> {
    regs: std::vec::IntoIter<R>,
    labels: &'a [Label],
}

impl<R> Placed<'_, R> {
    /// The register of the next binding.
    pub(crate) fn take(&mut self) -> R {
        self.regs.next().expect("a register for each binding")
    }

    pub(crate) fn label(&self, line: vun_inline::Line) -> Label {
        self.labels[usize::from(line.0)]
    }
}

/// How an address reaches `target`: an import through its entry in the global offset table.
pub(crate) fn via(target: Address) -> Via {
    match target {
        Address::Import(_) => Via::Got,
        Address::Func(_) | Address::Data(_) => Via::Direct,
    }
}

/// A binding of a block, with its instruction and the kind of the position it is at: `None`
/// for an address's base.
pub(crate) struct Use<'a, A: Inline> {
    inst: &'a Inst,
    kind: Option<A::Kind>,
    bound: &'a Binding,
}

impl<A: Inline> Use<'_, A> {
    fn fixed(&self) -> Result<Option<A::Reg>, AsmOperand> {
        let named = self.bound.reg().map(|r| A::general(r).ok_or(AsmOperand::Unallocated)).transpose()?;
        A::fixed(self.kind, named)
    }
}

/// A block of inline assembly, as an architecture encodes it.
pub(crate) trait Inlined {
    /// Its bindings, in the order `Block::bindings` gives them.
    fn uses<A: Inline>(&self) -> impl Iterator<Item = Use<'_, A>>;

    /// Whether every memory operand is one `A` addresses, and every binding has a register.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn check<A: Inline>(&self) -> Result<(), Lack>;

    /// Its inputs in registers, the addresses of its outputs, a register for each output, and
    /// the registers it names or its forms use unnamed kept for it, but where an input is read.
    /// A value recomputed where used takes a scratch register, or is written into the one it
    /// is fixed to.
    fn needs<A: Inline>(&self, cx: &Cx<'_, A>, needs: &mut Needs);

    /// Emits it: each output given its register, loaded for `inout`; each instruction by its
    /// form; then each output stored, where the block goes on.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn emit<A: Inline>(&self, cx: &mut Cx<'_, A>) -> Result<(), Lack>;
}

impl Inlined for Asm {
    fn uses<A: Inline>(&self) -> impl Iterator<Item = Use<'_, A>> {
        self.insts.iter().flat_map(|inst| {
            A::kinds(inst).zip(&inst.operands).flat_map(move |(kind, operand)| {
                let (kind, bounds) = match operand {
                    vun_inline::Operand::Bound(b) => (Some(kind), [Some(b), None]),
                    vun_inline::Operand::Mem(mem) => (None, mem.bindings()),
                    _ => (None, [None, None]),
                };
                bounds.into_iter().flatten().map(move |bound| Use { inst, kind, bound })
            })
        })
    }

    fn check<A: Inline>(&self) -> Result<(), Lack> {
        for inst in &self.insts {
            let lack = |o| Missing::AsmOperand(o).at(inst.origin);
            for (kind, operand) in A::kinds(inst).zip(&inst.operands) {
                if let vun_inline::Operand::Mem(mem) = operand {
                    A::addressing(mem, kind).map_err(lack)?;
                }
            }
        }
        for used in self.uses::<A>() {
            used.fixed().map_err(|o| Missing::AsmOperand(o).at(used.inst.origin))?;
        }
        Ok(())
    }

    fn needs<A: Inline>(&self, cx: &Cx<'_, A>, needs: &mut Needs) {
        let mut reserved = RegSet::EMPTY;
        let mut inputs = RegSet::EMPTY;
        for inst in &self.insts {
            let named = inst.operands.iter().filter_map(|operand| match operand {
                vun_inline::Operand::Reg(r)
                | vun_inline::Operand::Mem(vun_inline::Mem {
                    base: Some(vun_inline::Base::Reg(r)), ..
                }) => A::general(*r),
                _ => None,
            });
            reserved = A::implicit(inst).chain(named).fold(reserved, |set, r| set.with(A::preg(r)));
        }
        for used in self.uses::<A>() {
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

    fn emit<A: Inline>(&self, cx: &mut Cx<'_, A>) -> Result<(), Lack> {
        let mut regs = Vec::new();
        let mut outputs = Vec::new();
        for used in self.uses::<A>() {
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
        let mut placed = Placed { regs: regs.into_iter(), labels: &labels };
        for (inst, &label) in self.insts.iter().zip(&labels) {
            cx.asm.bind(label);
            let operands =
                A::kinds(inst).zip(&inst.operands).map(|(kind, o)| A::operand(o, kind, &mut placed));
            let operands: Vec<_> = operands.collect();
            A::emit(&mut cx.asm, inst, &operands)
                .map_err(|Untaken| Missing::AsmOperand(AsmOperand::Unencodable).at(inst.origin))?;
        }
        cx.asm.bind(labels[self.insts.len()]);
        if self.returns {
            for (r, addr, size) in outputs {
                A::store_at(&mut cx.asm, addr, r, size);
            }
        }
        Ok(())
    }
}

/// A memory operand of a block.
trait Addressed {
    /// The bindings of its base and its index, in that order.
    fn bindings(&self) -> [Option<&Binding>; 2];
}

impl Addressed for Mem {
    fn bindings(&self) -> [Option<&Binding>; 2] {
        let index = self.index.as_ref().map(|(index, _)| index);
        [self.base.as_ref(), index].map(|r| match r? {
            vun_inline::Base::Bound(b) => Some(b),
            vun_inline::Base::Reg(_) => None,
        })
    }
}

/// The size an output of type `ty` is loaded and stored at, or what the backend lacks for it.
fn sized<S: Width>(ty: Scalar, inst: &Inst) -> Result<S, Lack> {
    size_of(ty).ok_or_else(|| {
        let missing = if ty == Scalar::I128 { Missing::Int128 } else { Missing::Half };
        missing.at(inst.origin)
    })
}
