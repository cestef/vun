//! Outlining inline assembly (spec 13, section 3), for a backend that cannot write a block in the
//! function holding it: each block moves into an internal function of its own, which takes what
//! the block reads as parameters and the address of each place it writes, and the block's place
//! in the caller becomes a call of it. The function holding the rest of the code then needs
//! nothing of the backend that cannot express the block, and only the small functions do.
//!
//! A block stays where it is when it names a register itself (`Block::names_registers`), which is
//! the caller's state it reads or writes, and in a naked function, whose code is its blocks. A
//! function keeps all its blocks when one stays, since the backend would still not express it.
//! So does a function whose blocks and calls are only what its asm needs around it: moving its
//! block would leave a function that does nothing but call another. A function bound by
//! `#[stack.max]` keeps its blocks too, since a call would add to the bound.

#[cfg(test)]
mod tests;

use vun_index::Idx;
use vun_inline::Bound;
use vun_lir::func::Function;
use vun_lir::{
    AbiType, Alias, Asm, AsmCheck, Builder, Callee, Conv, Ext, FuncId, InstKind, Linkage, Scalar, Signature,
    Terminator, Unit,
};
use vun_origin::OriginId;

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
        let names = |&i: &_| matches!(&self.insts[i].kind, InstKind::Asm(asm, _) if asm.names_registers());
        if insts.iter().any(names) {
            return;
        }
        for inst in insts {
            let InstKind::Asm(asm, check) = &self.insts[inst].kind else { continue };
            let (thunk, args) = self.thunk(asm, *check, self.insts[inst].origin);
            let callee = Callee::Func(FuncId::new(first + thunks.len()));
            thunks.push(thunk);
            self.insts[inst].kind = InstKind::Call { callee, args };
        }
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
