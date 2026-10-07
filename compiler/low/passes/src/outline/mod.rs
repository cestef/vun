//! Outlining inline assembly (spec 13, section 3), for a backend that cannot write a block in the
//! function holding it: each block moves into an internal function of its own, which takes what
//! the block reads as parameters and the address of each place it writes, and the block's place
//! in the caller becomes a call of it. The function holding the rest of the code then needs
//! nothing of the backend that cannot express the block, and only the small functions do.
//!
//! A block stays where it is when it names a register that is the caller's state (`Registered::keeps`:
//! the stack, frame and link registers) and not scratch, and in a naked function, whose code is its blocks. A
//! function keeps all its blocks when one stays, since the backend would still not express it, and
//! when two name the same register, which may carry state from one to the other.
//! So does a function whose blocks and calls are only what its asm needs around it: moving its
//! block would leave a function that does nothing but call another. A function bound by
//! `#[stack.max]` keeps its blocks too, since a call would add to the bound.

#[cfg(test)]
mod tests;

use vun_index::Idx;
use vun_inline::{Bound, RegRef};
use vun_isa::{Blocks, Registered};
use vun_isa_arm64::Arm64;
use vun_isa_thumb::Thumb;
use vun_isa_x86_64::X86_64;
use vun_lir::func::Function;
use vun_lir::{
    AbiType, Alias, Asm, AsmCheck, Builder, CallKind, Callee, Conv, Ext, FuncId, Inst, InstKind, Linkage,
    Scalar, Signature, Terminator, Unit,
};
use vun_origin::OriginId;

/// A block of one of the instruction sets whose registers are known.
trait Keeps {
    /// Whether the block reads or writes state of the function holding it.
    fn keeps(&self) -> bool;

    /// The general registers the block names, or its forms use unnamed.
    fn names(&self) -> Vec<RegRef>;
}

impl Keeps for Asm {
    fn keeps(&self) -> bool {
        match self.arch {
            X86_64::ARCH => X86_64::keeps(self),
            Arm64::ARCH => Arm64::keeps(self),
            Thumb::ARCH => Thumb::keeps(self),
            _ => false,
        }
    }

    fn names(&self) -> Vec<RegRef> {
        match self.arch {
            X86_64::ARCH => X86_64::named(self).map(X86_64::reference).collect(),
            Arm64::ARCH => Arm64::named(self).map(Arm64::reference).collect(),
            Thumb::ARCH => Thumb::named(self).map(Thumb::reference).collect(),
            _ => Vec::new(),
        }
    }
}

pub trait Outline {
    /// Moves each block of inline assembly that may move into a function of its own, returning
    /// how many were.
    fn outline(&mut self) -> usize;
}

impl Outline for Unit {
    fn outline(&mut self) -> usize {
        let first = self.functions.len();
        let mut thunks = Vec::new();
        for (_, f) in self.functions.iter_mut() {
            f.outline_into(first, &mut thunks);
        }
        let moved = thunks.len();
        vun_trace::tracing::debug!(moved, functions = first, "outlined blocks of assembly");
        for thunk in thunks {
            self.functions.push(thunk);
        }
        if moved > 0 {
            self.verified();
        }
        moved
    }
}

trait Moves {
    /// Replaces each block of this function that may move by a call of a function holding it,
    /// which is pushed on `thunks`, the unit's `first` functions being before them.
    fn outline_into(&mut self, first: usize, thunks: &mut Vec<Function>);

    /// Whether two of the blocks `insts` name the same register: one may pass state to the other
    /// through it, which separate functions would not keep.
    fn shares_registers(&self, insts: &[Inst]) -> bool;

    /// Whether the function has anything of its own besides its blocks of assembly.
    fn holds_more(&self) -> bool;

    /// The function holding `asm`, written at `origin`, and the values it is called with.
    fn thunk(&self, asm: &Asm, check: Option<AsmCheck>, origin: OriginId) -> (Function, Vec<vun_lir::Value>);

    /// How a parameter of this function of type `ty` is passed.
    fn passed(ty: Scalar) -> AbiType;
}

impl Moves for Function {
    fn outline_into(&mut self, first: usize, thunks: &mut Vec<Function>) {
        if self.signature.conv == Conv::Naked || self.stack_max.is_some() || !self.holds_more() {
            return;
        }
        let insts: Vec<_> = self.blocks.iter().flat_map(|(_, block)| block.insts.iter().copied()).collect();
        let names = |&i: &_| matches!(&self.insts[i].kind, InstKind::Asm(asm, _) if asm.keeps());
        if insts.iter().any(names) || self.shares_registers(&insts) {
            return;
        }
        for inst in insts {
            let InstKind::Asm(asm, check) = &self.insts[inst].kind else { continue };
            let (thunk, args) = self.thunk(asm, *check, self.insts[inst].origin);
            let callee = Callee::Func(FuncId::new(first + thunks.len()));
            thunks.push(thunk);
            self.insts[inst].kind = InstKind::Call { callee, args, kind: CallKind::Plain };
        }
    }

    fn shares_registers(&self, insts: &[Inst]) -> bool {
        let mut seen = Vec::new();
        for &inst in insts {
            let InstKind::Asm(asm, _) = &self.insts[inst].kind else { continue };
            let mut names = asm.names();
            names.dedup();
            if names.iter().any(|r| seen.contains(r)) {
                return true;
            }
            seen.extend(names);
        }
        false
    }

    fn holds_more(&self) -> bool {
        self.blocks.len() > 1
            || self.insts.iter().any(|(_, inst)| {
                matches!(
                    inst.kind,
                    InstKind::Call { .. }
                        | InstKind::Copy { .. }
                        | InstKind::Rmw { .. }
                        | InstKind::CompareExchange { .. }
                )
            })
    }

    fn thunk(&self, asm: &Asm, check: Option<AsmCheck>, origin: OriginId) -> (Function, Vec<vun_lir::Value>) {
        let params = asm
            .bindings()
            .map(|bound| match bound {
                Bound::In { value, .. } => Self::passed(self.ty(*value)),
                Bound::Out { .. } | Bound::InOut { .. } => AbiType::Address(Alias::Any),
            })
            .collect();
        let args = asm.bindings().map(|bound| *bound.bound()).collect();
        let signature = Signature { conv: Conv::Vun, params, result: None, variadic: None, convention: None };
        let mut b = Builder::new(Linkage::Internal, signature, origin);
        let entry = b.current();
        let values = b.params(entry).to_vec();
        let mut moved = asm.clone();
        for (bound, value) in moved.bindings_mut().zip(values) {
            *bound.bound_mut() = value;
        }
        b.push(InstKind::Asm(Box::new(moved), check), None, origin);
        b.terminate(if asm.returns { Terminator::Return(None) } else { Terminator::Unreachable });
        (b.finish(), args)
    }

    fn passed(ty: Scalar) -> AbiType {
        match ty {
            Scalar::Ptr => AbiType::Address(Alias::Any),
            ty if ty.is_int() => AbiType::Int(ty, Ext::Zero),
            ty => AbiType::Scalar(ty),
        }
    }
}
