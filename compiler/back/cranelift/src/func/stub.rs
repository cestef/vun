//! The entry of a function compiled in the tail convention (`Tails`): a function of the target's
//! own convention that calls the body, so that every other caller, and every address, reaches the
//! function as the target's convention passes to it. Wasmtime's trampolines do the same where its
//! tail-convention functions meet the host. The entry is a function of LIR with the signature of
//! the function that calls it and returns what it gives, lowered as any other: its parameters
//! decode the convention, and its call encodes the body's.

use vun_backend::Lack;
use vun_lir::{Address, Builder, CallKind, Callee, FuncId, Function, InstKind, Terminator};
use vun_origin::OriginId;

use super::Cx;
use crate::names::Named;

impl Cx<'_> {
    /// The entry of the function `id`, `f`, whose body is lowered, and the origins its source
    /// locations number, which continue `origins`.
    ///
    /// # Errors
    /// Returns what Cranelift lacks of the signature.
    pub(super) fn entry(
        &self,
        id: FuncId,
        f: &Function,
        origins: Vec<OriginId>,
    ) -> Result<(cranelift_codegen::ir::Function, Vec<OriginId>), Lack> {
        let plan = self.plan(Address::Func(id), f.origin)?.expect("a function has a signature");
        let mut b = Builder::new(f.linkage, f.signature.clone(), f.origin);
        let params = b.params(b.current()).to_vec();
        let call = InstKind::Call { callee: Callee::Func(id), args: params, kind: CallKind::Plain };
        let result = b.push(call, f.signature.returns(), f.origin);
        b.terminate(Terminator::Return(result));
        let entry = b.finish();
        self.lowered(Address::Func(id).name(), &entry, plan, Some(id), origins)
    }
}
